mod pipeline;
mod spotify;
mod webserver;

use crate::spotify::SpotifyDBusClient;
use anyhow::Error;
use gstreamer as gst;
use gstreamer::prelude::*;
use pipeline::StreamPipeline;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio;
use tokio::sync::watch;
use tokio::sync::Mutex;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let gst_pipeline = StreamPipeline::new()?;
    gst_pipeline.set_state(gst::State::Playing)?;

    // Use a Tokio task to manage the GStreamer bus messages asynchronously
    let bus = gst_pipeline
        .get_bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    tokio::spawn(async move {
        handle_gst_bus_messages(bus, gst_pipeline.pipeline.clone().into()).await;
    });

    let spotify_client = Arc::new(Mutex::new(
        SpotifyDBusClient::new().await.expect("Failed to connect to Spotify D-Bus"),
    ));

    let (sleep_timer_tx, sleep_timer_rx) = watch::channel::<Option<Instant>>(None);
    let sleep_start_time = Arc::new(Mutex::new(None));
    let music_volume = Arc::new(Mutex::new(gst_pipeline.music.volume.clone()));

    webserver::start_server(
        sleep_timer_tx,
        sleep_start_time.clone(),
        spotify_client.clone(),
    );

    // Spawn a separate task that listens to the sleep timer channel
    tokio::spawn(async move {
        monitor_sleep_timer(sleep_timer_rx, music_volume.clone(), spotify_client.clone()).await;
    });

    // Keep the runtime alive
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");

    Ok(())
}

async fn monitor_sleep_timer(
    mut sleep_timer_rx: watch::Receiver<Option<Instant>>,
    music_volume: Arc<Mutex<gst::Element>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
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
            time::sleep(duration_until_end).await;

            // Check if the sleep timer hasn't been updated again
            if sleep_timer_rx.borrow().clone() == Some(end_time) {
                // Proceed with volume reduction
                println!("Starting volume decrease");

                let mut interval = time::interval(Duration::from_millis(500));
                for step in 0..=100 {
                    let volume_level = 1.0 - (step as f64 * 0.01);

                    {
                        let music_volume = music_volume.lock().await;
                        music_volume.set_property("volume", volume_level);
                    }

                    interval.tick().await;

                    // Check if the sleep timer was updated during reduction
                    if sleep_timer_rx.borrow().clone() != Some(end_time) {
                        // If the timer was canceled or changed, restore the volume immediately
                        let music_volume = music_volume.lock().await;
                        music_volume.set_property("volume", 1.0);
                        println!("Volume restoration due to new timer");
                        break;
                    }
                }

                // If volume reduction completed without interruption, pause playback
                if sleep_timer_rx.borrow().clone() == Some(end_time) {
                    time::sleep(Duration::from_secs(1)).await;
                    println!("Pausing Spotify playback");
                    spotify_client.lock().await.send_player_message("Pause").await;

                    // Wait for 5 seconds before restoring volume to 1.0
                    time::sleep(Duration::from_secs(5)).await;
                    let music_volume = music_volume.lock().await;
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
