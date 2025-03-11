mod http;
mod pipeline;
mod spotify_client;
mod spotify_old;
mod spotify_player;
mod spotify_sink;
mod webserver;

use crate::http::start_http_server;
use crate::pipeline::spotify::SpotifyInputSourceSelector;
use crate::spotify_client::{SpotifyClient, UnauthenticatedSpotifyClient};
use crate::spotify_old::SpotifyDBusClient;
use crate::spotify_sink::SinkEvent;
use crate::SpotifyState::{Authenticated, Unauthenticated};
use anyhow::Error;
use futures::lock;
use gstreamer as gst;
use gstreamer::event::{FlushStart, FlushStop};
use gstreamer::prelude::*;
use gstreamer::EventType::SegmentDone;
use gstreamer::{
    event, ClockTime, Element, Event, EventType, Format, Pipeline, Segment, Structure,
};
use gstreamer_app::AppSrc;
use librespot::protocol::authentication::AccountType::Spotify;
use librespot_playback::decoder::AudioPacket;
use pipeline::MainPipeline;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread::spawn;
use std::time::{Duration, Instant};
use tokio;
use tokio::sync::watch;
use tokio::sync::Mutex;
use tokio::time;
use tokio::time::sleep;
use warp::hyper::Client;
use warp::path::end;

struct CareChordsServer {
    spotify: SpotifyState,
    pipeline: Arc<MainPipeline>,
}

impl CareChordsServer {
    pub fn new() -> Self {
        Self {
            spotify: Unauthenticated(Arc::new(SpotifyClient::new())),
            pipeline: Arc::new(MainPipeline::new().unwrap()),
        }
    }

    pub async fn start(&mut self) {
        log::info!("Starting CareChordsServer!");
        self.start_gstreamer();

        self.start_spotify().await;

        if let Authenticated(spot) = &self.spotify {
            start_http_server(spot.clone());
        }

        sleep(Duration::from_secs(1)).await;

        if let Authenticated(spot) = &self.spotify {
            // spot.playlist("4k20pM1VwL5FSHQtlOENx5").await;
            // // spot.playlist("4Kl21mcSdESNomCLQXO5DP").await;
            // // spot.playlist("123Phuf9VqCgVndrnKBKlN").await;
            // sleep(Duration::from_secs(10)).await;
            // log::info!("Pause");
            // spot.pause().await;
            //
            // sleep(Duration::from_secs(5)).await;
            // log::info!("Play");
            // // self.pipeline
            // //     .spotify
            // //     .app_source
            // //     .set_state(gst::State::Ready)
            // //     .unwrap();
            // //
            // // self.pipeline
            // //     .spotify
            // //     .app_source
            // //     .set_state(gst::State::Playing)
            // //     .unwrap();
            // spot.play().await;
        }
    }

    async fn start_spotify(&mut self) {
        if let Unauthenticated(spotify) = &self.spotify {
            match spotify
                .try_cache_authentication_with_discovery_fallback()
                .await
            {
                Ok(mut client) => {
                    log::info!("Authenticated with Spotify");

                    let receiver = client.audio_stream_channel().take().unwrap();
                    let app_src = self.pipeline.spotify.app_source.clone();
                    let pipeline = self.pipeline.pipeline.clone();

                    Self::push_audio_app_src(pipeline, app_src, receiver);

                    self.spotify = Authenticated(Arc::new(client));
                }
                Err(e) => {
                    log::error!("Failed to authenticate with Spotify: {}", e);
                }
            }
        }
    }

    fn start_gstreamer(&mut self) {
        log::info!("Starting GStreamer!");
        let bus = self
            .pipeline
            .get_bus()
            .expect("Pipeline without bus. Shouldn't happen!");
        let p = self.pipeline.pipeline.clone();

        tokio::spawn(async move {
            handle_gst_bus_messages(bus, p.into()).await;
        });

        self.pipeline
            .set_state(gst::State::Playing)
            .expect("Failed to set pipeline to Playing");

        let pipeline = self.pipeline.clone();
        // TODO: GStreamer won't start if the silent source is used, so for now start with the app source and switch after 1 second
        tokio::spawn(async move {
            sleep(Duration::from_secs(1)).await;
            if let Ok(mut val) = pipeline.spotify.input_source.lock() {
                *val = SpotifyInputSourceSelector::Spotify;
            }
        });
    }

    // Start a new thread that consumes all audio packets from the receiver and sends it to the app src
    fn push_audio_app_src(pipeline: Pipeline, app_src: Element, receiver: Receiver<SinkEvent>) {
        let app_src = app_src
            .dynamic_cast::<AppSrc>()
            .expect("Source element is not an AppSrc!");

        tokio::spawn(async move {
            let mut timestamp: u64 = 0;
            let mut last_stopped_time = *pipeline.clock().unwrap().time().unwrap();

            while let Ok(event) = receiver.recv() {
                match event {
                    SinkEvent::Start => {
                        let now: ClockTime = pipeline.clock().unwrap().time().unwrap();

                        timestamp += *now - last_stopped_time;
                    }
                    SinkEvent::Stop => {
                        last_stopped_time = *pipeline.clock().unwrap().time().unwrap();
                    }
                    SinkEvent::Packet(samples) => {
                        // Skip empty packets.
                        if samples.is_empty() {
                            continue;
                        }

                        // Calculate the total number of bytes.
                        let byte_len = samples.len() * std::mem::size_of::<f64>();
                        // Create a new buffer for the audio data.
                        let mut buffer = gst::Buffer::with_size(byte_len)
                            .expect("Failed to allocate buffer for audio data");
                        {
                            // Get a writable map of the buffer.
                            let buffer_mut = buffer.get_mut().unwrap();
                            let mut map = buffer_mut
                                .map_writable()
                                .expect("Failed to map buffer writable");
                            // Safety: samples are stored contiguously in memory.
                            let sample_bytes = unsafe {
                                std::slice::from_raw_parts(samples.as_ptr() as *const u8, byte_len)
                            };
                            map.copy_from_slice(sample_bytes);
                        }

                        let frames = (samples.len() as u64) / 2;
                        // Duration in nanoseconds: (frames / sample_rate) seconds converted to ns.
                        let duration_ns = frames * 1_000_000_000 / 44100;
                        {
                            let buffer_mut = buffer.get_mut().unwrap();
                            buffer_mut.set_pts(gst::ClockTime::from_nseconds(timestamp));
                            buffer_mut.set_duration(gst::ClockTime::from_nseconds(duration_ns));
                        }
                        // log::warn!("Pushed @ {}", timestamp);
                        timestamp += duration_ns;

                        match app_src.push_buffer(buffer) {
                            Err(err) => {
                                eprintln!("Failed to push buffer: {:?}", err);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // When no more packets are available, send an EOS event.
            if let Err(err) = app_src.end_of_stream() {
                eprintln!("Failed to send EOS: {:?}", err);
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_logger::SimpleLogger::new()
        .with_colors(true)
        .with_module_level("warp", log::LevelFilter::Warn)
        .with_module_level("hyper", log::LevelFilter::Warn)
        .with_module_level("libmdns", log::LevelFilter::Warn)
        .with_module_level("rustls", log::LevelFilter::Warn)
        .with_module_level("symphonia_format_ogg", log::LevelFilter::Warn)
        .with_module_level("librespot", log::LevelFilter::Warn)
        .with_module_level("librespot_core", log::LevelFilter::Warn)
        .with_module_level("h2", log::LevelFilter::Warn)
        .with_level(log::LevelFilter::Debug)
        .init()?;

    let mut server = CareChordsServer::new();
    server.start().await;

    // let gst_pipeline = StreamPipeline::new()?;
    // gst_pipeline.set_state(gst::State::Playing)?;
    //
    // // Use a Tokio task to manage the GStreamer bus messages asynchronously
    // let bus = gst_pipeline
    //     .get_bus()
    //     .expect("Pipeline without bus. Shouldn't happen!");
    //
    // tokio::spawn(async move {
    //     handle_gst_bus_messages(bus, gst_pipeline.pipeline.clone().into()).await;
    // });

    // let spotify_client = Arc::new(Mutex::new(
    //     SpotifyDBusClient::new()
    //         .await
    //         .expect("Failed to connect to Spotify D-Bus"),
    // ));
    //
    // let (sleep_timer_tx, sleep_timer_rx) = watch::channel::<Option<Instant>>(None);
    // let sleep_start_time = Arc::new(Mutex::new(None));
    // let music_volume = Arc::new(Mutex::new(gst_pipeline.music.volume.clone()));
    //
    // webserver::start_server(
    //     sleep_timer_tx,
    //     sleep_start_time.clone(),
    //     spotify_client.clone(),
    // );
    //
    // // Spawn a separate task that listens to the sleep timer channel
    // tokio::spawn(async move {
    //     monitor_sleep_timer(sleep_timer_rx, music_volume.clone(), spotify_client.clone()).await;
    // });

    // Keep the runtime alive
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");

    Ok(())
}

enum SpotifyState {
    Unauthenticated(Arc<UnauthenticatedSpotifyClient>),
    Authenticated(Arc<SpotifyClient>),
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

            log::debug!("Starting sleep timer, will end at {:?}", end_time);
            time::sleep(duration_until_end).await;

            // Check if the sleep timer hasn't been updated again
            if sleep_timer_rx.borrow().clone() == Some(end_time) {
                // Proceed with volume reduction
                log::debug!("Starting volume decrease");

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
                        log::debug!("Volume restoration due to new timer");
                        break;
                    }
                }

                // If volume reduction completed without interruption, pause playback
                if sleep_timer_rx.borrow().clone() == Some(end_time) {
                    time::sleep(Duration::from_secs(1)).await;
                    log::debug!("Pausing Spotify playback");
                    spotify_client
                        .lock()
                        .await
                        .send_player_message("Pause")
                        .await;

                    // Wait for 5 seconds before restoring volume to 1.0
                    time::sleep(Duration::from_secs(5)).await;
                    let music_volume = music_volume.lock().await;
                    music_volume.set_property("volume", 1.0);
                    log::debug!("Volume restored after playback pause");
                }
            }
        }
    }
}

async fn handle_gst_bus_messages(bus: gst::Bus, pipeline: gst::Element) {
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => {
                log::error!("End of stream reached");
                break;
            }
            gst::MessageView::Error(err) => {
                log::error!(
                    "Error from {}: {}",
                    err.src()
                        .map(|s| s.path_string())
                        .unwrap_or_else(|| "None".into()),
                    err.error()
                );
                break;
            }
            // gst::MessageView::StreamStatus(s) => {log::info!("Received stream status: {:?}", s);}
            _ => (),
        }
    }

    // Clean up the pipeline
    if let Err(e) = pipeline.set_state(gst::State::Null) {
        log::error!("Failed to set pipeline state to Null: {}", e);
    }
}
