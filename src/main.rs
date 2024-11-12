mod pipeline;
mod spotify;

use crate::spotify::{is_spotify_playing, transfer_playback, MusicMetadata};
use anyhow::Error;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_audio::prelude::AudioDecoderExtManual;
use gstreamer_rtsp_server::prelude::RTSPMediaExt;
use pipeline::StreamPipeline;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use tokio;
use tokio::sync::watch;
use tokio::time;

use warp::http::StatusCode;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let gst_pipeline = StreamPipeline::new()?;
    gst_pipeline.set_state(gst::State::Playing)?;

    // Use a Tokio task to manage the GStreamer bus messages asynchronously
    let bus = gst_pipeline
        .get_bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    let pipeline_clone = gst_pipeline.pipeline.clone();
    tokio::spawn(async move {
        handle_gst_bus_messages(bus, pipeline_clone.into()).await;
    });

    let (sleep_timer_tx, sleep_timer_rx) = watch::channel::<Option<Instant>>(None);
    let sleep_start_time = Arc::new(Mutex::new(None));
    let music_volume = Arc::new(Mutex::new(gst_pipeline.music.volume.clone()));

    // Spawn a separate task that listens to the sleep timer channel
    let music_volume_clone = music_volume.clone();
    tokio::spawn(async move {
        monitor_sleep_timer(sleep_timer_rx, music_volume_clone).await;
    });

    let sleep_route = warp::path("sleep")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_sleep_sender(sleep_timer_tx.clone()))
        .and(with_sleep_time(sleep_start_time.clone()))
        .and_then(handle_sleep_request);

    let control_route = warp::path("control")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_sleep_time(sleep_start_time.clone()))
        .and_then(handle_control_request);

    let playback_route = warp::path("play")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handle_playback_request);

    // Update the state route to pass the sleep_start_time
    let state_route = warp::path("status")
        .and(warp::get())
        .and(with_sleep_time(sleep_start_time.clone()))
        .and_then(handle_state_request);

    // Update the routes to include the new /state route
    let routes = sleep_route
        .or(control_route)
        .or(playback_route)
        .or(state_route);

    tokio::spawn(async move {
        println!("Starting server @ :7755");
        warp::serve(routes).run(([0, 0, 0, 0], 7755)).await;
    });

    // Keep the runtime alive
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct PlayerStateDto {
    playing: bool,
    metadata: Option<MusicMetadata>,
    sleep_timer: Option<u64>,
}
async fn monitor_sleep_timer(
    mut sleep_timer_rx: watch::Receiver<Option<Instant>>,
    music_volume: Arc<Mutex<gst::Element>>,
) {
    while sleep_timer_rx.changed().await.is_ok() {
        let sleep_end_time = sleep_timer_rx.borrow().clone();

        if let Some(end_time) = sleep_end_time {
            let now = Instant::now();
            if now >= end_time {
                // The sleep timer has already expired
                continue;
            }

            let duration_until_end = end_time - now;

            println!("Starting sleep timer, will end at {:?}", end_time);

            // Wait until the end time
            time::sleep(duration_until_end).await;

            // Check if the sleep timer hasn't been updated again
            if sleep_timer_rx.borrow().clone() == Some(end_time) {
                // Proceed with volume reduction
                println!("Starting volume decrease");

                let mut interval = time::interval(Duration::from_millis(500));
                for step in 0..=100 {
                    let volume_level = 1.0 - (step as f64 * 0.01);

                    {
                        let mut music_volume = music_volume.lock().unwrap();
                        music_volume.set_property("volume", volume_level);
                    }

                    interval.tick().await;

                    // Check if the sleep timer was updated during reduction
                    if sleep_timer_rx.borrow().clone() != Some(end_time) {
                        // If the timer was canceled or changed, restore the volume immediately
                        let mut music_volume = music_volume.lock().unwrap();
                        music_volume.set_property("volume", 1.0);
                        println!("Volume restoration due to new timer");
                        break;
                    }
                }

                // If volume reduction completed without interruption, pause playback
                if sleep_timer_rx.borrow().clone() == Some(end_time) {
                    time::sleep(Duration::from_secs(1)).await;
                    println!("Pausing Spotify playback");
                    spotify::send_spotify_message("Pause");

                    // Wait for 5 seconds before restoring volume to 1.0
                    time::sleep(Duration::from_secs(5)).await;
                    let mut music_volume = music_volume.lock().unwrap();
                    music_volume.set_property("volume", 1.0);
                    println!("Volume restored after playback pause");
                }
            }
        }
    }
}

async fn handle_gst_bus_messages(bus: gst::Bus, pipeline: gst::Element) {
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => {
                println!("End of stream reached");
                break;
            }
            gst::MessageView::Error(err) => {
                eprintln!(
                    "Error from {}: {}",
                    err.src()
                        .map(|s| s.path_string())
                        .unwrap_or_else(|| "None".into()),
                    err.error()
                );
                break;
            }
            _ => (),
        }
    }
    // Clean up the pipeline
    if let Err(e) = pipeline.set_state(gst::State::Null) {
        eprintln!("Failed to set pipeline state to Null: {}", e);
    }
}

fn with_volume(
    volume: Arc<Mutex<gst::Element>>,
) -> impl Filter<Extract = (Arc<Mutex<gst::Element>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || volume.clone())
}

fn with_sleep_sender(
    sender: watch::Sender<Option<Instant>>,
) -> impl Filter<Extract = (watch::Sender<Option<Instant>>,), Error = std::convert::Infallible> + Clone
{
    warp::any().map(move || sender.clone())
}

fn with_sleep_time(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
) -> impl Filter<Extract = (Arc<Mutex<Option<Instant>>>,), Error = std::convert::Infallible> + Clone
{
    warp::any().map(move || sleep_start_time.clone())
}

async fn handle_playback_request(body: Value) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(uri) = body.get("uri").and_then(|t| t.as_str()) {
        transfer_playback();
        sleep(Duration::from_millis(500));
        spotify::playback(uri);

        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "status": "ok" })),
            StatusCode::OK,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
        StatusCode::BAD_REQUEST,
    ))
}

async fn handle_control_request(
    body: Value,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(state) = body.get("action").and_then(|t| t.as_str()) {
        match state.to_lowercase().as_str() {
            "play" => {
                transfer_playback();
                sleep(Duration::from_millis(500));
                spotify::send_spotify_message("Play");
            }
            "pause" => spotify::send_spotify_message("Pause"),
            "next" => spotify::send_spotify_message("Next"),
            "previous" => spotify::send_spotify_message("Previous"),
            _ => {}
        }

        let state = create_playerstate_dto(sleep_start_time).await;

        return Ok(warp::reply::with_status(
            warp::reply::json(&state),
            StatusCode::OK,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
        StatusCode::BAD_REQUEST,
    ))
}

async fn handle_state_request(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = create_playerstate_dto(sleep_start_time).await;

    Ok(warp::reply::with_status(
        warp::reply::json(&state),
        StatusCode::OK,
    ))
}


async fn create_playerstate_dto(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
) -> PlayerStateDto {
    let playing = is_spotify_playing();
    let music = spotify::get_spotify_metadata();

    // Calculate remaining sleep time if the timer is active
    let sleep_time_left = {
        let lock = sleep_start_time.lock().unwrap();
        if let Some(end_time) = *lock {
            let now = Instant::now();
            if now < end_time {
                Some((end_time - now).as_secs())
            } else {
                None
            }
        } else {
            None
        }
    };

    PlayerStateDto {
        playing,
        metadata: music,
        sleep_timer: sleep_time_left,
    }
}

async fn handle_sleep_request(
    body: Value,
    sleep_tx: watch::Sender<Option<Instant>>,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(sleep_timer) = body.get("timer").and_then(|t| t.as_u64()) {
        if !spotify::is_spotify_playing() {
            spotify::send_spotify_message("Play");
        }

        let end_time = Instant::now() + Duration::from_secs(sleep_timer);

        // Update sleep_start_time to the new end time
        let mut start_time_lock = sleep_start_time.lock().unwrap();
        *start_time_lock = Some(end_time);

        // Send the new end time to the channel, canceling the old timer
        let _ = sleep_tx.send(Some(end_time));

        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "status": "timer started" })),
            StatusCode::OK,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
        StatusCode::BAD_REQUEST,
    ))
}

async fn handle_volume_reduction(sleep_timer: u64, music_volume: Arc<Mutex<gst::Element>>) {
    println!("Starting sleep timer in {} seconds", sleep_timer);

    // Wait for the specified timer duration
    time::sleep(Duration::from_secs(sleep_timer)).await;

    println!("Starting volume decrease");

    // Gradually reduce the volume over interval
    let mut interval = time::interval(Duration::from_millis(500));
    for step in 0..=100 {
        let volume_level = 1.0 - (step as f64 * 0.01);
        {
            let music_volume = music_volume.lock().unwrap();
            music_volume.set_property("volume", volume_level);
        }
        interval.tick().await;
    }

    // Wait for 1 second before executing the bash script
    time::sleep(Duration::from_secs(1)).await;
    println!("Pausing Spotify playback");
    spotify::send_spotify_message("Pause");

    // Wait for 5 seconds before restoring the volume back to 1.0
    time::sleep(Duration::from_secs(5)).await;
    {
        let music_volume = music_volume.lock().unwrap();
        music_volume.set_property("volume", 1.0);
    }

    println!("Volume restored");
}
