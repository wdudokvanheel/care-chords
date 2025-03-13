use crate::pipeline::AudioPipeline;
use crate::server::SpotifyState::Authenticated;
use crate::spotify_client::{SpotifyClient, UnauthenticatedSpotifyClient};
use crate::spotify_player::SpotifyPlayerInfo;
use crate::spotify_sink::SinkEvent;
use crate::webserver::start_http_server;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer::{ClockTime, Element, Pipeline};
use gstreamer_app::AppSrc;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use tokio;
use tokio::sync::watch;
use tokio::task;
use crate::app_settings::ApplicationSettings;

pub enum SpotifyState {
    Unauthenticated(Arc<UnauthenticatedSpotifyClient>),
    Authenticated(Arc<SpotifyClient>),
}

pub struct CareChordsServer {
    spotify: SpotifyState,
    pipeline: Arc<AudioPipeline>,
}

impl CareChordsServer {
    pub fn new(settings: &ApplicationSettings) -> Self {
        Self {
            spotify: SpotifyState::Unauthenticated(Arc::new(SpotifyClient::new())),
            pipeline: Arc::new(AudioPipeline::new(&settings).unwrap()),
        }
    }

    pub async fn start(&mut self) {
        log::info!("Starting CareChordsServer!");
        self.start_gstreamer();
        self.start_spotify().await;

        if let Authenticated(spot) = &self.spotify {
            start_http_server(spot.clone());
        }
    }

    async fn start_spotify(&mut self) {
        if let SpotifyState::Unauthenticated(spotify) = &self.spotify {
            match spotify
                .try_cache_authentication_with_discovery_fallback()
                .await
            {
                Ok(mut spotify_client) => {
                    log::info!("Authenticated with Spotify");

                    let receiver = spotify_client.audio_stream_channel().take().unwrap();
                    let app_src = self.pipeline.spotify.app_source.clone();
                    let pipeline = self.pipeline.gstreamer_pipeline.clone();

                    Self::push_audio_app_src(pipeline, app_src, receiver);
                    Self::watch_events(spotify_client.player_info_channel());
                    self.spotify = Authenticated(Arc::new(spotify_client));
                }
                Err(e) => {
                    log::error!("Failed to authenticate with Spotify: {}", e);
                }
            }
        }
    }

    fn watch_events(receiver: watch::Receiver<SpotifyPlayerInfo>) {
        let mut receiver = receiver;

        task::spawn(async move {
            while receiver.changed().await.is_ok() {
                println!("{:?}", *receiver.borrow());
            }
        });
    }

    fn start_gstreamer(&mut self) {
        log::info!("Starting GStreamer!");
        let bus = self
            .pipeline
            .get_bus()
            .expect("Pipeline without bus. Shouldn't happen!");
        let pipeline = self.pipeline.gstreamer_pipeline.clone();

        tokio::spawn(async move {
            handle_gst_bus_messages(bus, pipeline.into()).await;
        });

        self.pipeline
            .set_state(gst::State::Playing)
            .expect("Failed to set pipeline to Playing");
    }

    // Start a new thread that consumes all audio packets from librespot's audio sink and sends it to the gstreamer app src
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


async fn handle_gst_bus_messages(bus: gst::Bus, pipeline: Element) {
    for msg in bus.iter_timed(ClockTime::NONE) {
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
            gst::MessageView::Warning(warn) => {
                log::error!("{:?}", warn);
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
