use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_data_dir(path: impl Into<PathBuf>) {
    let _ = DATA_DIR.set(path.into());
}

pub fn data_dir() -> PathBuf {
    DATA_DIR
        .get()
        .cloned()
        .or_else(|| env::var("CARECHORDS_DATA_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn credentials_file() -> PathBuf {
    data_dir().join("credentials.json")
}

pub fn legacy_credentials_file() -> PathBuf {
    cache_dir().join("credentials.json")
}

pub fn playlist_metadata_cache_dir() -> PathBuf {
    cache_dir().join("playlists")
}

pub fn system_playlists_file() -> PathBuf {
    cache_dir().join("system_playlists.json")
}
