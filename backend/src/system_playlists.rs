use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPlaylist {
    pub id: String,
    pub name: String,
    pub items: Vec<SystemPlaylistItem>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPlaylistItem {
    pub id: String,
    pub source: String,
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSystemPlaylistRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddSystemPlaylistItemRequest {
    pub source: String,
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub title: String,
}

#[derive(Clone)]
pub struct SystemPlaylistStore {
    path: PathBuf,
    playlists: Arc<Mutex<Vec<SystemPlaylist>>>,
}

impl SystemPlaylistStore {
    pub fn new(path: PathBuf) -> Self {
        let playlists = read_playlists(&path).unwrap_or_else(|e| {
            log::warn!("Failed to load system playlists: {e}");
            Vec::new()
        });

        Self {
            path,
            playlists: Arc::new(Mutex::new(playlists)),
        }
    }

    pub fn list(&self) -> Vec<SystemPlaylist> {
        self.playlists.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<SystemPlaylist> {
        self.playlists
            .lock()
            .unwrap()
            .iter()
            .find(|playlist| playlist.id == id)
            .cloned()
    }

    pub fn create(&self, name: String) -> Result<SystemPlaylist> {
        let now = now_epoch_secs();
        let playlist = SystemPlaylist {
            id: format!("playlist-{now}-{}", rand::random::<u32>()),
            name,
            items: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        {
            let mut playlists = self.playlists.lock().unwrap();
            playlists.push(playlist.clone());
            self.persist_locked(&playlists)?;
        }

        Ok(playlist)
    }

    pub fn add_item(&self, playlist_id: &str, item: AddSystemPlaylistItemRequest) -> Result<SystemPlaylist> {
        let mut playlists = self.playlists.lock().unwrap();
        let playlist = playlists
            .iter_mut()
            .find(|playlist| playlist.id == playlist_id)
            .ok_or_else(|| anyhow!("Unknown system playlist: {playlist_id}"))?;
        playlist.items.push(SystemPlaylistItem {
            id: format!("item-{}-{}", now_epoch_secs(), rand::random::<u32>()),
            source: item.source,
            kind: item.kind,
            reference: item.reference,
            title: item.title,
        });
        playlist.updated_at = now_epoch_secs();
        let updated = playlist.clone();
        self.persist_locked(&playlists)?;
        Ok(updated)
    }

    fn persist_locked(&self, playlists: &[SystemPlaylist]) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let json = serde_json::to_vec_pretty(playlists)?;
        fs::write(&self.path, json)
            .with_context(|| format!("Failed to write {}", self.path.display()))?;
        Ok(())
    }
}

fn read_playlists(path: &PathBuf) -> Result<Vec<SystemPlaylist>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let playlists = serde_json::from_slice(&bytes)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(playlists)
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
