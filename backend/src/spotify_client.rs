use futures::StreamExt;

use crate::spotify_player::{PlayerCommand, SpotifyPlayer, SpotifyPlayerInfo};
use crate::spotify_sink::SinkEvent;
use anyhow::{anyhow, Result};
use http::Method;
use librespot::core::SessionConfig;
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::DeviceType;
use librespot_core::Session;
use librespot_discovery::Discovery;
use librespot_core::SpotifyId;
use librespot_core::SpotifyUri;
use librespot_metadata::{Metadata, Playlist};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::BufReader;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tokio::sync::watch;

pub struct UnauthenticatedSpotifyClient {
    cache_folder: PathBuf,
}

pub struct SpotifyClient {
    audio_channel_receiver: Mutex<Option<std::sync::mpsc::Receiver<SinkEvent>>>,
    player_command_channel: Sender<PlayerCommand>,
    player_info_channel: watch::Receiver<SpotifyPlayerInfo>,
    session: Session,
}

#[derive(Debug, Serialize)]
pub struct PlaylistSummary {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RootlistResponse {
    #[serde(default)]
    items: Vec<RootlistItem>,
    #[serde(default)]
    contents: Option<RootlistContents>,
    #[serde(default)]
    next_offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RootlistContents {
    #[serde(default)]
    items: Vec<RootlistItem>,
}

#[derive(Debug, Deserialize)]
struct RootlistItem {
    uri: String,
}

#[derive(Debug, Default, Deserialize)]
struct UserProfileResponse {
    #[serde(default)]
    playlists: UserProfilePlaylists,
    #[serde(default)]
    public_playlists: Option<Vec<PublicPlaylistItem>>,
}

#[derive(Debug, Default, Deserialize)]
struct UserProfilePlaylists {
    #[serde(default)]
    items: Vec<UserProfilePlaylistItem>,
}

#[derive(Debug, Deserialize)]
struct UserProfilePlaylistItem {
    uri: String,
    name: String,
    #[serde(default)]
    images: Vec<UserProfileImage>,
    #[serde(default)]
    image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublicPlaylistItem {
    uri: String,
    name: String,
    #[serde(default)]
    image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserProfileImage {
    url: Option<String>,
}

impl SpotifyClient {
    pub fn new() -> UnauthenticatedSpotifyClient {
        UnauthenticatedSpotifyClient {
            cache_folder: PathBuf::from("cache"),
        }
    }
    /// This channel can push commands to the player
    pub fn player_command_channel(&self) -> Sender<PlayerCommand> {
        self.player_command_channel.clone()
    }

    /// This channel will emit the current state of the player
    pub fn player_info_channel(&self) -> watch::Receiver<SpotifyPlayerInfo> {
        self.player_info_channel.clone()
    }

    /// This channel provides audio samples and audio stream status updates
    pub fn audio_stream_channel(&self) -> Option<std::sync::mpsc::Receiver<SinkEvent>> {
        self.audio_channel_receiver.lock().unwrap().take()
    }

    pub async fn playlists(&self) -> Result<Vec<PlaylistSummary>> {
        // First try to collect from the profile API (gives names/images for public playlists).
        let mut by_uri = self.fetch_profile_playlist_map().await.unwrap_or_default();

        // Then augment with the rootlist (may include private playlists); fetch names via metadata when missing.
        if let Ok(root_uris) = self.fetch_rootlist_uris().await {
            for uri in root_uris {
                if by_uri.contains_key(&uri) {
                    continue;
                }
                if let Some(meta) = self.fetch_playlist_metadata(&uri).await {
                    by_uri.insert(uri.clone(), meta);
                } else {
                    log::warn!("Failed to fetch metadata for playlist uri={uri}; skipping");
                }
            }
        }

        let mut playlists: Vec<PlaylistSummary> = by_uri.into_values().collect();
        playlists.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(playlists)
    }

    async fn fetch_rootlist_uris(&self) -> Result<Vec<String>> {
        let mut uris = Vec::new();
        let mut offset = 0;
        let limit = 200;
        let username = self.session.username();

        loop {
            let endpoint = format!(
                "/playlist/v2/user/{username}/rootlist?response-format=json&limit={limit}&offset={offset}"
            );
            log::info!("Fetching playlists (rootlist) with endpoint: {endpoint}");
            let response = self
                .session
                .spclient()
                .request_as_json(&Method::GET, &endpoint, None, None)
                .await
                .map_err(|e| anyhow!(e))?;

            let rootlist: RootlistResponse = serde_json::from_slice(&response).map_err(|e| {
                let snippet = String::from_utf8_lossy(&response);
                anyhow!("Failed to parse rootlist response: {e}; body_snippet={}", &snippet[..snippet.len().min(500)])
            })?;

            let has_items = !rootlist.items.is_empty();
            let content_items = rootlist
                .contents
                .as_ref()
                .map(|c| c.items.len())
                .unwrap_or(0);
            log::info!(
                "Rootlist page fetched: {} top-level items, {} content items, next_offset={:?}",
                rootlist.items.len(),
                content_items,
                rootlist.next_offset
            );
            let returned_items = if !rootlist.items.is_empty() {
                rootlist.items
            } else {
                rootlist
                    .contents
                    .map(|c| c.items)
                    .unwrap_or_else(Vec::new)
            };
            if returned_items.is_empty() {
                let snippet = String::from_utf8_lossy(&response);
                log::info!(
                    "Rootlist response body (truncated to 500 chars): {}",
                    &snippet[..snippet.len().min(500)]
                );
            }

            uris.extend(returned_items.into_iter().map(|item| item.uri));

            match rootlist.next_offset {
                Some(next) if has_items => offset = next,
                _ => break,
            }
        }

        Ok(uris)
    }

    async fn fetch_profile_playlist_map(&self) -> Result<HashMap<String, PlaylistSummary>> {
        let username = self.session.username();
        let limit = 200;
        let endpoint = format!(
            "/user-profile-view/v3/profile/{username}?playlist_limit={limit}&artist_limit=0"
        );
        log::info!("Fetching playlists (profile fallback) with endpoint: {endpoint}");

        let response = self
            .session
            .spclient()
            .get_user_profile(&username, Some(limit), Some(0))
            .await
            .map_err(|e| anyhow!(e))?;

        let profile: UserProfileResponse = serde_json::from_slice(&response).map_err(|e| {
            let snippet = String::from_utf8_lossy(&response);
            anyhow!("Failed to parse profile response: {e}; body_snippet={}", &snippet[..snippet.len().min(500)])
        })?;
        log::info!(
            "Profile playlists fetched: {} items",
            profile.playlists.items.len()
        );
        if profile.playlists.items.is_empty() {
            let snippet = String::from_utf8_lossy(&response);
            log::info!(
                "Profile response body (truncated to 500 chars): {}",
                &snippet[..snippet.len().min(500)]
            );
        }

        let mut map = HashMap::new();
        for item in profile.playlists.items {
            map.insert(
                item.uri.clone(),
                PlaylistSummary {
                    uri: item.uri,
                    name: item.name,
                    image_uri: item.images.get(0).and_then(|img| img.url.clone()).or(item.image_url),
                },
            );
        }
        for item in profile.public_playlists.unwrap_or_default() {
            map.entry(item.uri.clone()).or_insert(PlaylistSummary {
                uri: item.uri,
                name: item.name,
                image_uri: item.image_url,
            });
        }

        Ok(map)
    }

    async fn fetch_playlist_metadata(&self, uri: &str) -> Option<PlaylistSummary> {
        let parsed = SpotifyUri::from_uri(uri).ok()?;
        let playlist = Playlist::get(&self.session, &parsed).await.ok()?;

        Some(PlaylistSummary {
            uri: uri.to_string(),
            name: playlist.name().to_string(),
            image_uri: None,
        })
    }
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

        let player = SpotifyPlayer::new(session.clone(), sender);
        let command_channel = player.command_channel();
        let info_channel = player.player_info_channel();

        tokio::spawn(async move {
            player.start().await;
        });

        SpotifyClient {
            audio_channel_receiver: Mutex::new(Some(receiver)),
            player_command_channel: command_channel,
            player_info_channel: info_channel,
            session,
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

fn create_spotify_cache() -> Option<Cache> {
    let credentials_path = Some("cache");
    let volume_path = Some("cache");
    let audio_path = Some("cache");
    let size_limit = Some(1024 * 1024 * 1024);

    Cache::new(credentials_path, volume_path, audio_path, size_limit).ok()
}
