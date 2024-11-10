mod pipeline;
mod spotify;

use crate::spotify::{is_spotify_playing, transfer_playback, MusicMetadata};
use anyhow::Error;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_audio::prelude::AudioDecoderExtManual;
use pipeline::StreamPipeline;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio;
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

    let music_volume = Arc::new(Mutex::new(gst_pipeline.music.volume.clone()));

    let sleep_route = warp::path("sleep")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_volume(music_volume.clone()))
        .and_then(handle_sleep_request);

    let control_route = warp::path("control")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handle_control_request);

    let playback_route = warp::path("play")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handle_playback_request);

    let state_route = warp::path("status")
        .and(warp::get())
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

async fn handle_playback_request(body: Value) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(uri) = body.get("uri").and_then(|t| t.as_str()) {
        spotify::playback(uri);
        transfer_playback();
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

async fn handle_control_request(body: Value) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(state) = body.get("state").and_then(|t| t.as_str()) {
        match state.to_lowercase().as_str() {
            "play" => spotify::send_spotify_message("Play"),
            "pause" => spotify::send_spotify_message("Pause"),
            "next" => spotify::send_spotify_message("Next"),
            "previous" => spotify::send_spotify_message("Previous"),
            _ => {}
        }

        // Get the current state after handling the control request
        let playing = is_spotify_playing();
        let music = spotify::get_spotify_metadata();
        let current_state = PlayerStateDto { playing, metadata: music };

        return Ok(warp::reply::with_status(
            warp::reply::json(&current_state),
            StatusCode::OK,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
        StatusCode::BAD_REQUEST,
    ))
}

async fn handle_state_request() -> Result<impl warp::Reply, warp::Rejection> {
    let playing = is_spotify_playing();
    let music = spotify::get_spotify_metadata();
    let state = PlayerStateDto { playing, metadata: music };

    Ok(warp::reply::with_status(
        warp::reply::json(&state),
        StatusCode::OK,
    ))
}

async fn handle_sleep_request(
    body: Value,
    music_volume: Arc<Mutex<gst::Element>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(sleep_timer) = body.get("timer").and_then(|t| t.as_u64()) {
        if !spotify::is_spotify_playing() {
            spotify::send_spotify_message("Play");
        }

        // Spawn a new task to handle the timer and volume reduction
        tokio::spawn(handle_volume_reduction(sleep_timer, music_volume));

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
