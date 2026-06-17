use futures::StreamExt;

use crate::data_paths;
use crate::spotify_player::{PlayerCommand, SpotifyPlayer, SpotifyPlayerInfo};
use crate::spotify_sink::SinkEvent;
use anyhow::{Result, anyhow};
use hex::encode as hex_encode;
use http::Method;
use librespot::core::SessionConfig;
use librespot_core::Session;
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;
use librespot_core::config::DeviceType;
use librespot_discovery::Discovery;

use librespot_core::SpotifyUri;
use librespot_core::error::ErrorKind;
use librespot_metadata::{Metadata, Playlist, Track};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex as TokioMutex, RwLock, watch};

const PLAYLIST_METADATA_REQUEST_SPACING: Duration = Duration::from_millis(250);
const PLAYLIST_ARTWORK_REQUEST_SPACING: Duration = Duration::from_millis(500);
pub struct UnauthenticatedSpotifyClient {
    audio_sender: Option<SyncSender<SinkEvent>>,
}

pub struct SpotifyClient {
    audio_channel_receiver: Mutex<Option<std::sync::mpsc::Receiver<SinkEvent>>>,
    player_command_channel: Sender<PlayerCommand>,
    player_info_channel: watch::Receiver<SpotifyPlayerInfo>,
    session: Session,
    playlists_cache: Arc<RwLock<Option<Vec<PlaylistSummary>>>>,
    playlists_fetch_lock: Arc<TokioMutex<()>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize)]
struct PlaylistMetadataCache {
    uri: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_uri: Option<String>,
    #[serde(default)]
    artwork_lookup_attempted: bool,
}

impl PlaylistMetadataCache {
    fn from_summary(summary: &PlaylistSummary, artwork_lookup_attempted: bool) -> Self {
        Self {
            uri: summary.uri.clone(),
            name: summary.name.clone(),
            image_uri: summary.image_uri.clone(),
            artwork_lookup_attempted,
        }
    }

    fn into_summary(self) -> PlaylistSummary {
        PlaylistSummary {
            uri: self.uri,
            name: self.name,
            image_uri: self.image_uri,
            folder: None,
        }
    }
}

impl SpotifyClient {
    pub fn new() -> UnauthenticatedSpotifyClient {
        UnauthenticatedSpotifyClient { audio_sender: None }
    }

    pub fn new_with_sender(sender: SyncSender<SinkEvent>) -> UnauthenticatedSpotifyClient {
        UnauthenticatedSpotifyClient {
            audio_sender: Some(sender),
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

        self.refresh_playlists().await
    }

    pub async fn refresh_playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let _fetch_guard = self.playlists_fetch_lock.lock().await;

        // First try to collect from the profile API (gives names/images for public playlists).
        let mut by_uri = match self.fetch_profile_playlist_map().await {
            Ok(playlists) => playlists,
            Err(e) => {
                log::warn!("Failed to fetch profile playlists: {e}");
                HashMap::new()
            }
        };

        // Then augment with the rootlist (may include private playlists or folder grouping).
        match self.fetch_rootlist_entries().await {
            Ok(root_entries) => {
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

                let mut missing = Vec::new();
                for (uri, folder) in to_fetch {
                    if let Some(cached) = read_playlist_metadata_cache(&uri) {
                        let mut summary = cached.into_summary();
                        summary.folder = folder.clone();
                        by_uri.insert(uri, summary);
                    } else {
                        missing.push((uri, folder));
                    }
                }

                if !missing.is_empty() {
                    log::info!("Fetching metadata for {} uncached playlists", missing.len());
                }

                for (uri, folder) in missing {
                    let meta = self.fetch_playlist_metadata(&uri).await;
                    if let Some(mut meta) = meta {
                        write_playlist_metadata_cache(&PlaylistMetadataCache::from_summary(
                            &meta,
                            meta.image_uri.is_some(),
                        ));
                        if meta.folder.is_none() {
                            meta.folder = folder;
                        }
                        by_uri.insert(uri, meta);
                    } else {
                        log::warn!("Failed to fetch metadata for playlist uri={uri}; skipping");
                    }
                    tokio::time::sleep(PLAYLIST_METADATA_REQUEST_SPACING).await;
                }
            }
            Err(e) => {
                log::warn!("Failed to fetch rootlist playlists: {e}");
                if by_uri.is_empty() {
                    return Err(e);
                }
            }
        }

        let mut playlists: Vec<PlaylistSummary> = by_uri.into_values().collect();
        playlists.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Update cache
        *self.playlists_cache.write().await = Some(playlists.clone());

        Ok(playlists)
    }

    pub fn spawn_playlist_artwork_refresh(self: Arc<Self>) {
        tokio::spawn(async move {
            self.refresh_missing_playlist_artwork().await;
        });
    }

    async fn refresh_missing_playlist_artwork(&self) {
        let playlists = match self.playlists().await {
            Ok(playlists) => playlists,
            Err(e) => {
                log::warn!("Skipping playlist artwork refresh: {e}");
                return;
            }
        };

        let mut missing = Vec::new();
        for playlist in playlists {
            if playlist.image_uri.is_some() {
                continue;
            }

            match read_playlist_metadata_cache(&playlist.uri) {
                Some(cache) if cache.artwork_lookup_attempted => {}
                _ => missing.push(playlist),
            }
        }

        if missing.is_empty() {
            return;
        }

        log::info!("Fetching artwork for {} cached playlists", missing.len());

        for mut playlist in missing {
            playlist.image_uri = self.fetch_playlist_artwork(&playlist.uri).await;
            write_playlist_metadata_cache(&PlaylistMetadataCache::from_summary(&playlist, true));

            if playlist.image_uri.is_some() {
                let mut cache = self.playlists_cache.write().await;
                if let Some(playlists) = cache.as_mut() {
                    if let Some(existing) = playlists.iter_mut().find(|p| p.uri == playlist.uri) {
                        existing.image_uri = playlist.image_uri.clone();
                    }
                }
            }

            tokio::time::sleep(PLAYLIST_ARTWORK_REQUEST_SPACING).await;
        }
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
            let response = retry_rate_limited("rootlist API", || {
                self.session
                    .spclient()
                    .request_as_json(&Method::GET, &endpoint, None, None)
            })
            .await
            .map_err(|e| anyhow!(e))?;

            let rootlist: RootlistResponse = serde_json::from_slice(&response).map_err(|e| {
                let snippet = String::from_utf8_lossy(&response);
                anyhow!(
                    "Failed to parse rootlist response: {e}; body_snippet={}",
                    &snippet[..snippet.len().min(500)]
                )
            })?;

            let has_items = !rootlist.items.is_empty();
            let mut returned_items = if !rootlist.items.is_empty() {
                rootlist.items
            } else {
                rootlist.contents.map(|c| c.items).unwrap_or_else(Vec::new)
            };
            if returned_items.is_empty() {}

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

                let folder = folder_stack.last().map(|(_, name)| name.clone());
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

        let response = retry_rate_limited("profile playlists API", || {
            self.session
                .spclient()
                .get_user_profile(&username, Some(limit), Some(0))
        })
        .await
        .map_err(|e| anyhow!(e))?;

        let profile: UserProfileResponse = serde_json::from_slice(&response).map_err(|e| {
            let snippet = String::from_utf8_lossy(&response);
            anyhow!(
                "Failed to parse profile response: {e}; body_snippet={}",
                &snippet[..snippet.len().min(500)]
            )
        })?;
        if profile.playlists.items.is_empty() {}

        let mut map = HashMap::new();
        for item in profile.playlists.items {
            let summary = PlaylistSummary {
                uri: item.uri,
                name: item.name,
                image_uri: normalize_image(
                    item.images
                        .iter()
                        .find_map(|img| img.url.clone())
                        .or(item.image_url),
                ),
                folder: item.folder.clone(),
            };
            write_playlist_metadata_cache(&PlaylistMetadataCache::from_summary(
                &summary,
                summary.image_uri.is_some(),
            ));
            map.insert(summary.uri.clone(), summary);
        }
        for item in profile.public_playlists.unwrap_or_default() {
            let summary = PlaylistSummary {
                uri: item.uri,
                name: item.name,
                image_uri: normalize_image(item.image_url),
                folder: None,
            };
            write_playlist_metadata_cache(&PlaylistMetadataCache::from_summary(
                &summary,
                summary.image_uri.is_some(),
            ));
            map.entry(summary.uri.clone()).or_insert(summary);
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

        let playlist =
            match retry_rate_limited("Playlist::get", || Playlist::get(&self.session, &parsed))
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("Failed to get playlist '{}' from librespot: {}", uri, e);
                    return None;
                }
            };

        let image = playlist_cover(&playlist);

        Some(PlaylistSummary {
            uri: uri.to_string(),
            name: playlist.name().to_string(),
            image_uri: image,
            folder: None,
        })
    }

    async fn fetch_playlist_artwork(&self, uri: &str) -> Option<String> {
        let playlist_id = uri.strip_prefix("spotify:playlist:")?;
        let endpoint = format!("/playlist/v2/playlist/{}?response-format=json", playlist_id);
        let response = match retry_rate_limited("playlist artwork API", || {
            self.session
                .spclient()
                .request_as_json(&Method::GET, &endpoint, None, None)
        })
        .await
        {
            Ok(res) => res,
            Err(e) => {
                log::warn!("Artwork API request failed for playlist {playlist_id}: {e}");
                return None;
            }
        };

        let json: serde_json::Value = match serde_json::from_slice(&response) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Failed to parse artwork API response for {playlist_id}: {e}");
                return None;
            }
        };

        if let Some(image) = extract_playlist_image_from_json(&json) {
            return Some(image);
        }

        let mut collected_hashes = Vec::new();
        collect_track_uris(&json, &mut collected_hashes);

        for uri in collected_hashes {
            if let Ok(parsed_uri) = SpotifyUri::from_uri(&uri) {
                if let Ok(track) = retry_rate_limited("Track::get artwork", || {
                    Track::get(&self.session, &parsed_uri)
                })
                .await
                {
                    if let Some(image) = track.album.covers.first() {
                        let hex = image.id.to_string();
                        return normalize_image(Some(format!("spotify:image:{hex}")));
                    }
                }
            }
        }

        None
    }
}

/// Retry a librespot call that may fail with a client-side rate limit
/// (`ResourceExhausted`). Uses exponential backoff capped at 5s.
async fn retry_rate_limited<T, F, Fut>(label: &str, mut f: F) -> Result<T, librespot_core::Error>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, librespot_core::Error>>,
{
    const MAX_ATTEMPTS: u32 = 12;
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if e.kind == ErrorKind::ResourceExhausted && attempt < MAX_ATTEMPTS => {
                let backoff = Duration::from_millis((200u64 << attempt.min(5)).min(5000));
                log::warn!(
                    "{label} rate limited (attempt {}); retrying in {:?}",
                    attempt + 1,
                    backoff
                );
                tokio::time::sleep(backoff).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
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
        return Some(format!("https://mosaic.scdn.co/640/{mosaic_hash}")).or_else(|| {
            rest.split(':')
                .next()
                .map(|first| format!("https://i.scdn.co/image/{first}"))
        });
    }

    None
}

fn read_playlist_metadata_cache(uri: &str) -> Option<PlaylistMetadataCache> {
    let path = playlist_metadata_cache_path(uri);
    let file = File::open(&path).ok()?;
    let reader = BufReader::new(file);
    let cache: PlaylistMetadataCache = match serde_json::from_reader(reader) {
        Ok(cache) => cache,
        Err(e) => {
            log::warn!(
                "Failed to parse cached playlist metadata {}: {e}",
                path.display()
            );
            return None;
        }
    };

    if cache.uri != uri {
        log::warn!(
            "Ignoring cached playlist metadata {} with mismatched uri {}",
            path.display(),
            cache.uri
        );
        return None;
    }

    Some(cache)
}

fn write_playlist_metadata_cache(cache: &PlaylistMetadataCache) {
    let cache_dir = data_paths::playlist_metadata_cache_dir();
    if let Err(e) = fs::create_dir_all(&cache_dir) {
        log::warn!("Failed to create playlist metadata cache dir: {e}");
        return;
    }

    let path = playlist_metadata_cache_path(&cache.uri);

    let file = match File::create(&path) {
        Ok(file) => file,
        Err(e) => {
            log::warn!(
                "Failed to open playlist metadata cache {}: {e}",
                path.display()
            );
            return;
        }
    };

    if let Err(e) = serde_json::to_writer_pretty(file, cache) {
        log::warn!(
            "Failed to write playlist metadata cache {}: {e}",
            path.display()
        );
    }
}

fn playlist_metadata_cache_path(uri: &str) -> PathBuf {
    let digest = hex::encode(Sha1::digest(uri.as_bytes()));
    data_paths::playlist_metadata_cache_dir().join(format!("{digest}.json"))
}

fn extract_playlist_image_from_json(json: &serde_json::Value) -> Option<String> {
    let candidates = [
        "/attributes/image_url",
        "/attributes/picture",
        "/image_url",
        "/picture",
    ];

    for pointer in candidates {
        if let Some(url) = json.pointer(pointer).and_then(|v| v.as_str()) {
            if let Some(image) = normalize_image(Some(url.to_string())) {
                return Some(image);
            }
        }
    }

    if let Some(sizes) = json
        .pointer("/attributes/picture_sizes")
        .and_then(|v| v.as_array())
    {
        if let Some(image) = sizes
            .iter()
            .filter_map(|v| v.get("url").and_then(|s| s.as_str()))
            .find_map(|url| normalize_image(Some(url.to_string())))
        {
            return Some(image);
        }
    }

    if let Some(images) = json.get("images").and_then(|v| v.as_array()) {
        if let Some(image) = images
            .iter()
            .filter_map(|v| v.get("url").and_then(|s| s.as_str()))
            .find_map(|url| normalize_image(Some(url.to_string())))
        {
            return Some(image);
        }
    }

    None
}

fn collect_track_uris(json: &serde_json::Value, track_uris: &mut Vec<String>) {
    if track_uris.len() >= 4 {
        return;
    }

    match json {
        serde_json::Value::Object(map) => {
            if let Some(uri) = map.get("uri").and_then(|v| v.as_str()) {
                if uri.starts_with("spotify:track:") && !track_uris.iter().any(|u| u == uri) {
                    track_uris.push(uri.to_string());
                }
            }

            for value in map.values() {
                collect_track_uris(value, track_uris);
                if track_uris.len() >= 4 {
                    break;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_track_uris(value, track_uris);
                if track_uris.len() >= 4 {
                    break;
                }
            }
        }
        _ => {}
    }
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
            Ok(creds) => Ok(self.authenticate(creds).await?),
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

        Ok(Self::from_authenticated_session(
            session,
            self.audio_sender.clone(),
        ))
    }

    fn from_authenticated_session(
        session: Session,
        external_sender: Option<SyncSender<SinkEvent>>,
    ) -> SpotifyClient {
        let (sender, receiver) = if let Some(s) = external_sender {
            (s, None)
        } else {
            let (s, r) = sync_channel::<SinkEvent>(10);
            (s, Some(r))
        };

        let player = SpotifyPlayer::new(session.clone(), sender);
        let command_channel = player.command_channel();
        let info_channel = player.player_info_channel();

        tokio::spawn(async move {
            player.start().await;
        });

        SpotifyClient {
            audio_channel_receiver: Mutex::new(receiver),
            player_command_channel: command_channel,
            player_info_channel: info_channel,
            session,
            playlists_cache: Arc::new(RwLock::new(None)),
            playlists_fetch_lock: Arc::new(TokioMutex::new(())),
        }
    }

    pub async fn fetch_credentials_from_cache(&self) -> Result<Credentials> {
        let path = [
            data_paths::credentials_file(),
            data_paths::legacy_credentials_file(),
        ]
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No Spotify credentials found at {} or {}",
                data_paths::credentials_file().display(),
                data_paths::legacy_credentials_file().display()
            )
        })?;

        log::info!("Loading Spotify credentials from: {}", path.display());

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
    let credentials_path = data_paths::data_dir();
    let cache_path = data_paths::cache_dir();
    let size_limit = Some(1024 * 1024 * 1024);

    Cache::new(
        Some(credentials_path),
        Some(cache_path.clone()),
        Some(cache_path),
        size_limit,
    )
    .ok()
}
