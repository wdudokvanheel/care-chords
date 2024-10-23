mod pipeline;

use anyhow::Error;
use gstreamer as gst;
use gstreamer::prelude::*;
use pipeline::StreamPipeline;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio;
use tokio::process::Command;
use tokio::time;
use warp::http::StatusCode;
use warp::Filter;

use dbus::blocking::BlockingSender;
use dbus::blocking::Connection;
use dbus::Message;

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
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            match msg.view() {
                gst::MessageView::Eos(..) => {
                    println!("End of stream reached");
                    break;
                }
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Error from {}: {}",
                        err.src().map(|s| s.path_string()).unwrap_or_else(|| "None".into()),
                        err.error()
                    );
                    break;
                }
                _ => (),
            }
        }

        // Clean up the pipeline
        pipeline_clone.set_state(gst::State::Null).unwrap();
    });

    let music_volume = gst_pipeline.music.volume.clone();
    let music_volume = Arc::new(Mutex::new(music_volume));

    let control_route = warp::path("sleep")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |body: Value| {
            let music_volume_clone = Arc::clone(&music_volume);
            async move {
                if let Some(time) = body.get("timer") {
                    if let Some(sleep_timer) = time.as_u64() {
                        // Valid timer, let's start the timer and resume playback (might be paused, otherwise will be ignored)
                        println!("Executing bash script to play Spotify");
                        send_spotify_message("Play");

                        // Spawn a new task to handle the timer and volume reduction
                        let music_volume_clone = Arc::clone(&music_volume_clone);
                        tokio::spawn(async move {
                            println!("Starting sleep timer in {} seconds", sleep_timer);

                            // Wait for the specified timer duration
                            time::sleep(Duration::from_secs(sleep_timer)).await;

                            println!("Starting volume decrease");

                            // Gradually reduce the volume over interval
                            let mut interval = time::interval(Duration::from_millis(500));
                            for step in 0..=100 {
                                let volume_level = 1.0 - (step as f64 * 0.01);
                                {
                                    let music_volume = music_volume_clone.lock().unwrap();
                                    music_volume.set_property("volume", volume_level);
                                }
                                interval.tick().await;
                            }

                            // Wait for 1 second before executing the bash script
                            time::sleep(Duration::from_secs(1)).await;
                            println!("Executing bash script to pause Spotify");
                            send_spotify_message("Pause");


                            // Wait for 5 seconds before restoring the volume back to 1.0
                            time::sleep(Duration::from_secs(5)).await;
                            {
                                let music_volume = music_volume_clone.lock().unwrap();
                                music_volume.set_property("volume", 1.0);
                            }

                            println!("Volume restored");
                        });

                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "status": "timer started" })),
                            StatusCode::OK,
                        ));
                    }
                }
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
                    StatusCode::BAD_REQUEST,
                ))
            }
        });
    let routes = control_route;

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

pub fn send_spotify_message(message: &str) {
    // Establish a connection to the D-Bus session
    let conn = Connection::new_session()
        .expect("Failed to create a dbus connection");

    // Call ListNames to get all service names on the session bus
    let proxy = conn.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", Duration::from_millis(5000));
    let (names, ): (Vec<String>,) = proxy.method_call("org.freedesktop.DBus", "ListNames", ())
        .expect("Failed to list dbus names");

    // Find the Spotify destination that contains the word 'spotify'
    let spotify_dest = names.iter().find(|name| name.contains("org.mpris.MediaPlayer2.spotify"));

    // Check if Spotify destination is found
    let spotify_dest = match spotify_dest {
        Some(dest) => dest,
        None => {
            eprintln!("Spotify destination not found.");
            return;
        }
    };

    // Send the Pause command to Spotify
    let pause_msg = Message::new_method_call(
        spotify_dest,
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
        message,
    )
        .expect("Failed to call dbus method");

    let response = conn.send_with_reply_and_block(pause_msg, Duration::from_millis(5000))
        .expect("Failed to send message to dbus");
    println!("Pause command sent, response: {:?}", response);
}

