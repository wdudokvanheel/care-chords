use crate::pipeline::AudioPipeline;
use crate::pipeline::audio_bridge::AudioBridge;
use crate::local_audio::{LocalAudioLibrary, LocalAudioPlayer};
use crate::playback_controller::PlaybackController;
use crate::spotify_client::{SpotifyClient, UnauthenticatedSpotifyClient};
use crate::spotify_player::SpotifyPlayerInfo;
use crate::spotify_sink::SinkEvent;
use crate::system_playlists::SystemPlaylistStore;
use crate::webserver::start_http_server;

use crate::app_settings::ApplicationSettings;
use gstreamer as gst;
use gstreamer::prelude::{Cast, ElementExt, GstObjectExt};
use gstreamer::{ClockTime, Element};
use gstreamer_app::AppSrc;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use tokio;
use tokio::sync::watch;
use tokio::task;
use tokio::time::{Duration, timeout};

const SPOTIFY_STARTUP_AUTH_TIMEOUT: Duration = Duration::from_secs(8);

pub enum SpotifyState {
    Unauthenticated(Arc<UnauthenticatedSpotifyClient>),
    Authenticated(Arc<SpotifyClient>),
}

pub struct CareChordsServer {
    spotify: SpotifyState,
    pipeline: Arc<AudioPipeline>,
    monitor_url: String,
    local_library: LocalAudioLibrary,
    local_player: Arc<LocalAudioPlayer>,
    system_playlists: SystemPlaylistStore,
    audio_bridge: Arc<AudioBridge>,
}

impl CareChordsServer {
    pub fn new(settings: &ApplicationSettings) -> Self {
        let (sender, receiver) = sync_channel::<SinkEvent>(10);
        let audio_bridge = Arc::new(AudioBridge::new(receiver));

        Self {
            spotify: SpotifyState::Unauthenticated(Arc::new(SpotifyClient::new_with_sender(
                sender.clone(),
            ))),
            pipeline: Arc::new(AudioPipeline::new(&settings).unwrap()),
            monitor_url: settings.monitor_url.clone(),
            local_library: LocalAudioLibrary::new(&settings.local_audio),
            local_player: Arc::new(LocalAudioPlayer::new(sender.clone())),
            system_playlists: SystemPlaylistStore::new("cache/system_playlists.json".into()),
            audio_bridge,
        }
    }

    pub async fn start(&mut self) {
        log::info!("Starting CareChordsServer!");
        self.start_gstreamer();
        self.start_spotify().await;

        let spotify = match &self.spotify {
            SpotifyState::Authenticated(spotify) => Some(spotify.clone()),
            SpotifyState::Unauthenticated(_) => None,
        };
        let playback = PlaybackController::new(
            spotify,
            self.local_library.clone(),
            self.local_player.clone(),
            self.system_playlists.clone(),
        );
        start_http_server(Arc::new(playback), self.monitor_url.clone());
    }

    async fn start_spotify(&mut self) {
        if let SpotifyState::Unauthenticated(spotify) = &self.spotify {
            match timeout(
                SPOTIFY_STARTUP_AUTH_TIMEOUT,
                spotify.try_cache_authentication_with_discovery_fallback(),
            )
            .await
            {
                Ok(Ok(spotify_client)) => {
                    self.finish_spotify_authentication(spotify_client).await;
                }
                Ok(Err(e)) => {
                    log::error!("Failed to authenticate with Spotify: {}", e);
                }
                Err(_) => {
                    log::warn!(
                        "Spotify authentication timed out after {:?}; starting without Spotify",
                        SPOTIFY_STARTUP_AUTH_TIMEOUT
                    );
                }
            }
        }
    }

    async fn finish_spotify_authentication(&mut self, spotify_client: SpotifyClient) {
        log::info!("Authenticated with Spotify");

        Self::watch_events(spotify_client.player_info_channel());

        log::info!("Refreshing playlist cache...");
        if let Err(e) = spotify_client.refresh_playlists().await {
            log::warn!("Failed to refresh playlist cache: {e}");
        }

        let spotify_client = Arc::new(spotify_client);
        spotify_client.clone().spawn_playlist_artwork_refresh();

        self.spotify = SpotifyState::Authenticated(spotify_client);
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
        let pipeline = self.pipeline.clone();
        let audio_bridge = self.audio_bridge.clone();

        tokio::spawn(async move {
            loop {
                log::info!("Initializing GStreamer pipeline...");
                let bus = pipeline
                    .get_bus()
                    .expect("Pipeline without bus. Shouldn't happen!");

                // Update the bridge with the current app_src
                let app_src = pipeline
                    .spotify
                    .app_source
                    .clone()
                    .dynamic_cast::<AppSrc>()
                    .expect("Source element is not an AppSrc!");
                audio_bridge.set_app_src(app_src);

                // Start the pipeline
                if let Err(e) = pipeline.set_state(gst::State::Playing) {
                    log::error!("Failed to set pipeline to Playing: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }

                // Monitor the bus
                handle_gst_bus_messages(bus, pipeline.gstreamer_pipeline.clone().into()).await;

                // If we are here, the pipeline has stopped or failed.
                log::warn!("GStreamer pipeline stopped. Restarting in 1 second...");
                audio_bridge.clear_app_src();

                // Ensure pipeline is stopped before restarting
                let _ = pipeline.set_state(gst::State::Null);

                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
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
