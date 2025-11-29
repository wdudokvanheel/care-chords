use futures::{stream, StreamExt};

use crate::spotify_player::{PlayerCommand, SpotifyPlayer, SpotifyPlayerInfo};
use crate::spotify_sink::SinkEvent;
use anyhow::{anyhow, Result};
use hex::encode as hex_encode;
use http::Method;
use librespot::core::SessionConfig;
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::DeviceType;
use librespot_core::Session;
use librespot_discovery::Discovery;

use librespot_core::SpotifyUri;
use librespot_metadata::{Metadata, Playlist, Track};
use percent_encoding::percent_decode_str;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::BufReader;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tokio::sync::{watch, RwLock};
use std::sync::Arc;

pub struct UnauthenticatedSpotifyClient {
    cache_folder: PathBuf,
}

pub struct SpotifyClient {
    audio_channel_receiver: Mutex<Option<std::sync::mpsc::Receiver<SinkEvent>>>,
    player_command_channel: Sender<PlayerCommand>,
    player_info_channel: watch::Receiver<SpotifyPlayerInfo>,
    session: Session,
    playlists_cache: Arc<RwLock<Option<Vec<PlaylistSummary>>>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PlaylistSummary {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
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
    #[serde(default)]
    folder: Option<String>,
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

#[derive(Debug, Deserialize)]
struct OEmbedResponse {
    #[serde(default)]
    thumbnail_url: Option<String>,
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
        // Check cache first
        if let Some(cached) = self.playlists_cache.read().await.as_ref() {
            return Ok(cached.clone());
        }

        // First try to collect from the profile API (gives names/images for public playlists).
        let mut by_uri = self.fetch_profile_playlist_map().await.unwrap_or_default();

        // Then augment with the rootlist (may include private playlists or folder grouping).
        if let Ok(root_entries) = self.fetch_rootlist_entries().await {
            let mut to_fetch = Vec::new();

            for (uri, folder) in root_entries {
                if let Some(existing) = by_uri.get_mut(&uri) {
                    if existing.folder.is_none() {
                        existing.folder = folder;
                    }
                } else {
                    to_fetch.push((uri, folder));
                }
            }

            let fetched_metas = stream::iter(to_fetch)
                .map(|(uri, folder)| async move {
                    let meta = self.fetch_playlist_metadata(&uri).await;
                    (uri, folder, meta)
                })
                .buffer_unordered(10)
                .collect::<Vec<_>>()
                .await;

            for (uri, folder, meta_opt) in fetched_metas {
                if let Some(mut meta) = meta_opt {
                    if meta.folder.is_none() {
                        meta.folder = folder;
                    }
                    by_uri.insert(uri, meta);
                } else {
                    log::warn!("Failed to fetch metadata for playlist uri={uri}; skipping");
                }
            }
        }

        // Backfill missing images using metadata/oembed for any playlist that still lacks art.
        let missing_images: Vec<String> = by_uri
            .iter()
            .filter(|(_, p)| p.image_uri.is_none())
            .map(|(u, _)| u.clone())
            .collect();

        let fetched_images = stream::iter(missing_images)
            .map(|uri| async move {
                let meta = self.fetch_playlist_metadata(&uri).await;
                (uri, meta)
            })
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;

        for (uri, meta_opt) in fetched_images {
            if let Some(meta) = meta_opt {
                if let Some(img) = meta.image_uri {
                    if let Some(existing) = by_uri.get_mut(&uri) {
                        if existing.image_uri.is_none() {
                            existing.image_uri = Some(img);
                        }
                    }
                }
            }
        }

        let mut playlists: Vec<PlaylistSummary> = by_uri.into_values().collect();
        playlists.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        
        // Update cache
        *self.playlists_cache.write().await = Some(playlists.clone());
        
        Ok(playlists)
    }

    async fn fetch_rootlist_entries(&self) -> Result<Vec<(String, Option<String>)>> {
        let mut entries = Vec::new();
        let mut offset = 0;
        let limit = 200;
        let username = self.session.username();
        let mut folder_stack: Vec<(String, String)> = Vec::new(); // (group_id, name)

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
            let mut returned_items = if !rootlist.items.is_empty() {
                rootlist.items
            } else {
                rootlist
                    .contents
                    .map(|c| c.items)
                    .unwrap_or_else(Vec::new)
            };
            if returned_items.is_empty() {
            }

            for item in returned_items.drain(..) {
                if let Some((group_id, name)) = parse_start_group(&item.uri) {
                    folder_stack.push((group_id, name));
                    continue;
                }
                if let Some(end_id) = parse_end_group(&item.uri) {
                    if let Some(pos) = folder_stack.iter().rposition(|(id, _)| id == &end_id) {
                        folder_stack.truncate(pos);
                    } else {
                        folder_stack.pop();
                    }
                    continue;
                }

                let folder = folder_stack
                    .last()
                    .map(|(_, name)| name.clone());
                entries.push((item.uri, folder));
            }

            match rootlist.next_offset {
                Some(next) if has_items => offset = next,
                _ => break,
            }
        }

        Ok(entries)
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
        if profile.playlists.items.is_empty() {
        }

        let mut map = HashMap::new();
        for item in profile.playlists.items {
            map.insert(
                item.uri.clone(),
                PlaylistSummary {
                    uri: item.uri,
                    name: item.name,
                    image_uri: normalize_image(
                        item.images
                            .iter()
                            .find_map(|img| img.url.clone())
                            .or(item.image_url),
                    ),
                    folder: item.folder.clone(),
                },
            );
        }
        for item in profile.public_playlists.unwrap_or_default() {
            map.entry(item.uri.clone()).or_insert(PlaylistSummary {
                uri: item.uri,
                name: item.name,
                image_uri: normalize_image(item.image_url),
                folder: None,
            });
        }

        Ok(map)
    }

    async fn fetch_playlist_metadata(&self, uri: &str) -> Option<PlaylistSummary> {
        let parsed = match SpotifyUri::from_uri(uri) {
            Ok(p) => p,
            Err(e) => {
                log::error!("Invalid Spotify URI '{}': {}", uri, e);
                return None;
            }
        };

        let playlist = match Playlist::get(&self.session, &parsed).await {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Failed to get playlist '{}' from librespot: {}", uri, e);
                return None;
            }
        };

        let mut image = playlist_cover(&playlist);
        
        // Fallback to internal API if librespot metadata is missing cover
        if image.is_none() {
             if let Some(id) = uri.strip_prefix("spotify:playlist:") {
                 image = self.fetch_playlist_via_api(id).await;
             }
        }

        if image.is_none() {
            image = self.fetch_oembed_cover(uri).await;
            if image.is_none() {
                log::warn!("Failed to find cover for '{}' via OEmbed", playlist.name());
            }
        }

        Some(PlaylistSummary {
            uri: uri.to_string(),
            name: playlist.name().to_string(),
            image_uri: image,
            folder: None,
        })
    }

    async fn fetch_playlist_via_api(&self, playlist_id: &str) -> Option<String> {
        let endpoint = format!("/playlist/v2/playlist/{}?response-format=json", playlist_id);
        let response = match self.session.spclient().request_as_json(&Method::GET, &endpoint, None, None).await {
            Ok(res) => res,
            Err(e) => {
                log::warn!("API request failed for playlist {}: {}", playlist_id, e);
                return None;
            }
        };

        let json: serde_json::Value = match serde_json::from_slice(&response) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Failed to parse API response for playlist {}: {}", playlist_id, e);
                return None;
            }
        };

        // Try to find image in various locations
        // 1. attributes.picture_sizes
        if let Some(sizes) = json.pointer("/attributes/picture_sizes").and_then(|v| v.as_array()) {
            if let Some(url) = sizes.iter().filter_map(|v| v.get("url").and_then(|s| s.as_str())).next() {
                return normalize_image(Some(url.to_string()));
            }
        }

        // 2. images (root)
        if let Some(images) = json.get("images").and_then(|v| v.as_array()) {
             if let Some(url) = images.iter().filter_map(|v| v.get("url").and_then(|s| s.as_str())).next() {
                return normalize_image(Some(url.to_string()));
            }
        }
        
        // 3. attributes.image_url
        if let Some(url) = json.pointer("/attributes/image_url").and_then(|v| v.as_str()) {
            return normalize_image(Some(url.to_string()));
        }

        // 4. Fallback: Try to get the first track's album art
        if let Some(items) = json.pointer("/contents/items").and_then(|v| v.as_array()) {
            // DEBUG: Log JSON for Sleep Fantasy Forest
            if playlist_id == "4jrxitD9zZ4C1VNB19ksLb" {
                 let json_str = serde_json::to_string_pretty(&json).unwrap_or_default();
                 log::info!("Sleep Fantasy Forest JSON: {}", &json_str[..json_str.len().min(5000)]);
            }

            for item in items {
                if let Some(uri) = item.get("uri").and_then(|v| v.as_str()) {
                    if uri.starts_with("spotify:track:") {
                        if let Ok(parsed_uri) = SpotifyUri::from_uri(uri) {
                            if let Ok(track) = Track::get(&self.session, &parsed_uri).await {
                                if let Some(cover) = self.get_track_cover(&track) {
                                    return Some(cover);
                                }
                            }
                        }
                    }
                }
            }
        }

        let json_str = serde_json::to_string_pretty(&json).unwrap_or_default();
        None
    }

    fn get_track_cover(&self, track: &Track) -> Option<String> {
        if let Some(image) = track.album.covers.first() {
             let hex = image.id.to_string();
             return normalize_image(Some(format!("spotify:image:{hex}")));
        }
        None
    }

    async fn fetch_oembed_cover(&self, uri: &str) -> Option<String> {
        let http_url = if uri.starts_with("spotify:playlist:") {
            uri.replace("spotify:playlist:", "https://open.spotify.com/playlist/")
        } else {
            uri.to_string()
        };

        let client = Client::new();
        let url = format!(
            "https://open.spotify.com/oembed?url={}",
            urlencoding::encode(&http_url)
        );
        match client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    log::warn!(
                        "OEmbed request failed for '{}' (url={}): status {}",
                        uri,
                        http_url,
                        resp.status()
                    );
                    return None;
                }
                match resp.json::<OEmbedResponse>().await {
                    Ok(oembed) => normalize_image(oembed.thumbnail_url),
                    Err(e) => {
                        log::warn!("Failed to parse OEmbed response for '{}': {}", uri, e);
                        None
                    }
                }
            }
            Err(e) => {
                log::warn!("OEmbed network error for '{}': {}", uri, e);
                None
            }
        }
    }
}

fn parse_start_group(uri: &str) -> Option<(String, String)> {
    let prefix = "spotify:start-group:";
    if !uri.starts_with(prefix) {
        return None;
    }
    let rest = uri.trim_start_matches(prefix);
    let mut parts = rest.splitn(2, ':');
    let id = parts.next()?.to_string();
    let raw_name = parts.next().unwrap_or_default();
    let decoded = decode_group_name(raw_name);
    Some((id, decoded))
}

fn parse_end_group(uri: &str) -> Option<String> {
    let prefix = "spotify:end-group:";
    if !uri.starts_with(prefix) {
        return None;
    }
    Some(uri.trim_start_matches(prefix).to_string())
}

fn decode_group_name(raw: &str) -> String {
    let with_spaces = raw.replace('+', " ");
    percent_decode_str(&with_spaces)
        .decode_utf8()
        .map(|s| s.to_string())
        .unwrap_or(with_spaces)
}

fn normalize_image(raw: Option<String>) -> Option<String> {
    let uri = raw?;
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return Some(uri);
    }

    // Handle spotify:image:<hash> and spotify:mosaic:<hash1>:<hash2>:...
    let prefix_image = "spotify:image:";
    let prefix_mosaic = "spotify:mosaic:";
    if let Some(rest) = uri.strip_prefix(prefix_image) {
        return Some(format!("https://i.scdn.co/image/{rest}"));
    }
    if let Some(rest) = uri.strip_prefix(prefix_mosaic) {
        // Try the full mosaic hash (colon-separated) and fall back to the first tile.
        let mosaic_hash = rest.replace(':', "");
        return Some(format!("https://mosaic.scdn.co/640/{mosaic_hash}"))
            .or_else(|| rest.split(':').next().map(|first| format!("https://i.scdn.co/image/{first}")));
    }

    None
}

fn playlist_cover(playlist: &Playlist) -> Option<String> {
    // Prefer the picture hash if present.
    if let Some(pic_hash) = playlist
        .attributes
        .picture_sizes
        .iter()
        .filter_map(|p| Some(p.url.clone()))
        .next()
    {
        return normalize_image(Some(pic_hash));
    }

    if !playlist.attributes.picture.is_empty() {
        let hex = hex_encode(&playlist.attributes.picture);
        return normalize_image(Some(format!("spotify:image:{hex}")));
    }

    None
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
            playlists_cache: Arc::new(RwLock::new(None)),
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
