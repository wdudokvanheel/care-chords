use futures::StreamExt;

use crate::spotify_player::{PlayerCommand, SpotifyPlayer, SpotifyPlayerInfo};
use anyhow::{anyhow, Result};
use gstreamer::prelude::ObjectExt;
use librespot::core::SessionConfig;
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::DeviceType;
use librespot_core::Session;
use librespot_discovery::Discovery;
use librespot_playback::audio_backend::Sink;
use librespot_playback::decoder::AudioPacket;
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::watch;
use crate::spotify_sink::SinkEvent;


pub struct UnauthenticatedSpotifyClient {
    cache_folder: PathBuf,
}

pub struct SpotifyClient {
    session: Arc<Session>,
    audio_channel_sender: Option<SyncSender<AudioPacket>>,
    audio_channel_receiver: Option<std::sync::mpsc::Receiver<SinkEvent>>,
    player_command_channel: Sender<PlayerCommand>,
    player_info_channel: watch::Receiver<SpotifyPlayerInfo>,
}

impl UnauthenticatedSpotifyClient {
    pub async fn try_cache_authentication_with_discovery_fallback(&self) -> Result<SpotifyClient> {
        let credentials = self.fetch_credentials_from_cache().await;

        match credentials {
            Ok(creds) => {
                Ok(self.authenticate(creds).await?)
            }
            Err(_) => {
                log::info!("Failed to load credentials from cache, going in discovery mode");
                match self.discover_credentials().await {
                    Ok(creds) => self.authenticate(creds).await,
                    Err(e) => Err(anyhow!("Failed to get credentials from discovery: {}", e)),
                }
            }
        }
    }

    pub async fn authenticate(&self, credentials: Credentials) -> Result<SpotifyClient> {
        let cache = create_spotify_cache();
        let session_config = SessionConfig::default();
        let session = Session::new(session_config, cache);

        let _ = session.connect(credentials, false).await?;

        Ok(Self::from_authenticated_session(session))
    }

    fn from_authenticated_session(session: Session) -> SpotifyClient {
        let (sender, receiver) = sync_channel::<SinkEvent>(10);

        let mut player = SpotifyPlayer::new(session.clone(), sender);
        let command_channel = player.command_channel();
        let info_channel = player.player_info_channel();

        tokio::spawn(async move {
            player.start().await;
        });

        SpotifyClient {
            session: Arc::new(session),
            audio_channel_sender: None,
            audio_channel_receiver: Some(receiver),
            player_command_channel: command_channel,
            player_info_channel: info_channel,
        }
    }

    pub async fn fetch_credentials_from_cache(&self) -> Result<Credentials> {
        let path = self.cache_folder.join("credentials.json");
        log::info!("Loading cache from: {}", path.display());
        if !path.exists() {
            return Err(anyhow::anyhow!(format!(
                "File {} does not exist.",
                path.display()
            )));
        }

        let file =
            File::open(path).map_err(|e| anyhow::anyhow!(format!("Failed to open file: {}", e)))?;

        let reader = BufReader::new(file);
        let credentials: Credentials = serde_json::from_reader(reader)
            .map_err(|e| anyhow::anyhow!(format!("Failed to parse json: {}", e)))?;

        Ok(credentials)
    }

    pub async fn discover_credentials(&self) -> Result<Credentials> {
        let name = "Care Chords Setup";
        let device_id = hex::encode(Sha1::digest(name.as_bytes()));

        let mut discovery =
            Discovery::builder(device_id, "fc4ccd0248b948cb8a5f19d594dfba0d".to_string())
                .device_type(DeviceType::Speaker)
                .launch()
                .unwrap();

        log::info!("Searching for Spotify Connect devices");

        while let Some(credentials) = discovery.next().await {
            let cache = create_spotify_cache();

            let session_config = SessionConfig::default();
            let session = Session::new(session_config, cache);

            match session.connect(credentials.clone(), true).await {
                Ok(_) => {
                    log::info!(
                        "Found device: {}, saved credentials for {}",
                        session.device_id(),
                        session.username()
                    );
                    return Ok(credentials);
                }
                Err(_) => {
                    continue;
                }
            }
        }

        Err(anyhow!("Failed to get credentials"))
    }
}

impl SpotifyClient {
    pub fn new() -> UnauthenticatedSpotifyClient {
        UnauthenticatedSpotifyClient {
            cache_folder: PathBuf::from("cache"),
        }
    }

    // Use this channel to push commands to the player
    pub fn player_command_channel(&self) -> Sender<PlayerCommand> {
        self.player_command_channel.clone()
    }

    // Use this channel to get the current state of the player
    pub fn player_info_channel(&self) -> watch::Receiver<SpotifyPlayerInfo> {
        self.player_info_channel.clone()
    }

    pub fn audio_stream_channel(&mut self) -> Option<std::sync::mpsc::Receiver<SinkEvent>> {
        self.audio_channel_receiver.take()
    }
}

fn create_spotify_cache() -> Option<Cache> {
    let credentials_path = Some("cache");
    let volume_path = Some("cache");
    let audio_path = Some("cache");
    let size_limit = Some(1024 * 1024 * 1024);

    Cache::new(credentials_path, volume_path, audio_path, size_limit).ok()
}

pub const SCOPES: [&str; 13] = [
    "playlist-read-collaborative",
    "playlist-read-private",
    "playlist-modify-private",
    "playlist-modify-public",
    "user-follow-read",
    "user-follow-modify",
    "user-library-modify",
    "user-library-read",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-playback-state",
    "user-read-private",
    "user-read-recently-played",
];
