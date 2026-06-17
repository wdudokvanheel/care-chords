use crate::app_settings::LocalAudioSettings;
use crate::spotify_player::{MusicMetadata, SpotifyPlayerInfo, SpotifyPlayerState};
use crate::spotify_sink::SinkEvent;
use anyhow::{Context, Result, anyhow};
use gstreamer as gst;
use gstreamer::prelude::{Cast, ElementExt};
use gstreamer_app::AppSink;
use gstreamer_app::prelude::*;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey};
use symphonia::core::probe::Hint;
use tokio::sync::watch;

const LOCAL_FILE_PLAYBACK_VOLUME: f64 = 0.35;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAudioEntry {
    pub id: String,
    pub name: String,
    pub kind: LocalAudioEntryKind,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<LocalAudioMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalAudioMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalAudioEntryKind {
    File,
    Folder,
}

#[derive(Clone)]
pub struct LocalAudioLibrary {
    roots: Arc<Vec<PathBuf>>,
    allowed_extensions: Arc<HashSet<String>>,
    artwork_path: &'static str,
    root_listing: RootListing,
}

#[derive(Clone, Copy)]
enum RootListing {
    Roots,
    Albums,
}

pub struct LocalAudioPlayer {
    audio_sender: SyncSender<SinkEvent>,
    info_sender: watch::Sender<SpotifyPlayerInfo>,
    info_receiver: watch::Receiver<SpotifyPlayerInfo>,
    state: Arc<Mutex<LocalPlaybackState>>,
}

pub struct LocalPlaybackQueue {
    pub entries: Vec<LocalAudioEntry>,
    pub start_index: usize,
}

#[derive(Default)]
struct LocalPlaybackState {
    queue: Vec<LocalAudioEntry>,
    current_index: usize,
    cancel: Option<Arc<AtomicBool>>,
}

impl LocalAudioLibrary {
    pub fn new(settings: &LocalAudioSettings) -> Self {
        let roots = settings.roots.iter().map(PathBuf::from).collect::<Vec<_>>();
        let allowed_extensions = settings
            .allowed_extensions
            .iter()
            .map(|ext| ext.trim_start_matches('.').to_lowercase())
            .collect::<HashSet<_>>();

        Self {
            roots: Arc::new(roots),
            allowed_extensions: Arc::new(allowed_extensions),
            artwork_path: "/library/local/artwork",
            root_listing: RootListing::Albums,
        }
    }

    pub fn new_youtube(settings: &LocalAudioSettings) -> Self {
        let mut library = Self::new(settings);
        library.artwork_path = "/library/youtube/artwork";
        library.root_listing = RootListing::Roots;
        library
    }

    pub fn roots(&self) -> Vec<LocalAudioEntry> {
        self.roots
            .iter()
            .enumerate()
            .map(|(idx, root)| {
                let name = root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_else(|| root.to_str().unwrap_or("Music"));
                LocalAudioEntry {
                    id: format!("root:{idx}"),
                    name: name.to_string(),
                    kind: LocalAudioEntryKind::Folder,
                    path: format!("root:{idx}"),
                    image_uri: self.folder_image_uri(root),
                    metadata: None,
                }
            })
            .collect()
    }

    pub fn list(&self, requested_path: Option<&str>) -> Result<Vec<LocalAudioEntry>> {
        if requested_path.is_none() {
            return match self.root_listing {
                RootListing::Roots => Ok(self.roots()),
                RootListing::Albums => self.list_albums(),
            };
        }

        let folder = self.resolve_folder_ref(requested_path.unwrap())?;
        self.list_folder(&folder)
    }

    fn list_folder(&self, folder: &Path) -> Result<Vec<LocalAudioEntry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&folder)
            .with_context(|| format!("Failed to read local audio folder {}", folder.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let name = entry
                .file_name()
                .to_str()
                .map(|name| name.to_string())
                .unwrap_or_else(|| path.display().to_string());

            if path.is_dir() {
                let reference = self.path_to_ref(&path)?;
                entries.push(LocalAudioEntry {
                    id: reference.clone(),
                    name,
                    kind: LocalAudioEntryKind::Folder,
                    path: reference.clone(),
                    image_uri: folder_artwork(&path).map(|_| self.artwork_uri(&reference)),
                    metadata: None,
                });
            } else if self.is_audio_file(&path) {
                entries.push(self.file_entry_with_fallback(path, strip_extension(&name))?);
            }
        }

        entries.sort_by(compare_local_entries);
        Ok(entries)
    }

    fn list_albums(&self) -> Result<Vec<LocalAudioEntry>> {
        let mut entries = Vec::new();
        for root in self.roots.iter() {
            self.collect_album_entries(root, true, &mut entries)?;
        }

        entries.sort_by(compare_local_entries);
        Ok(entries)
    }

    fn collect_album_entries(
        &self,
        folder: &Path,
        is_root: bool,
        out: &mut Vec<LocalAudioEntry>,
    ) -> Result<()> {
        let mut child_folders = Vec::new();
        let mut direct_files = Vec::new();

        for entry in fs::read_dir(folder)
            .with_context(|| format!("Failed to read local audio folder {}", folder.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                child_folders.push(path);
            } else if self.is_audio_file(&path) {
                direct_files.push(path);
            }
        }

        if !direct_files.is_empty() {
            if is_root {
                for file in direct_files {
                    out.push(self.file_entry(file)?);
                }
            } else {
                let reference = self.path_to_ref(folder)?;
                let name = folder
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| folder.display().to_string());
                out.push(LocalAudioEntry {
                    id: reference.clone(),
                    name,
                    kind: LocalAudioEntryKind::Folder,
                    path: reference.clone(),
                    image_uri: folder_artwork(folder).map(|_| self.artwork_uri(&reference)),
                    metadata: None,
                });
            }
            return Ok(());
        }

        for child in child_folders {
            self.collect_album_entries(&child, false, out)?;
        }

        Ok(())
    }

    pub fn resolve_ref(&self, reference: &str) -> Result<PathBuf> {
        let path = self.resolve_path_ref(reference)?;
        if !path.exists() {
            anyhow::bail!("Local audio path does not exist: {}", path.display());
        }
        Ok(path)
    }

    pub fn resolve_to_files(&self, reference: &str) -> Result<Vec<LocalAudioEntry>> {
        let path = self.resolve_ref(reference)?;
        if path.is_file() {
            if !self.is_audio_file(&path) {
                anyhow::bail!(
                    "Local file is not an allowed audio type: {}",
                    path.display()
                );
            }
            return Ok(vec![self.file_entry(path)?]);
        }

        let mut files = Vec::new();
        self.collect_audio_files(&path, &mut files)?;
        files.sort_by(compare_local_entries);
        Ok(files)
    }

    pub fn resolve_playback_queue(&self, reference: &str) -> Result<LocalPlaybackQueue> {
        let path = self.resolve_ref(reference)?;
        if path.is_file() {
            if !self.is_audio_file(&path) {
                anyhow::bail!(
                    "Local file is not an allowed audio type: {}",
                    path.display()
                );
            }

            let album_folder = path
                .parent()
                .ok_or_else(|| anyhow!("Local audio file has no parent folder"))?;
            let mut entries = Vec::new();
            self.collect_direct_audio_files(album_folder, &mut entries)?;
            entries.sort_by(compare_local_entries);

            let selected_ref = self.path_to_ref(&path)?;
            let start_index = entries
                .iter()
                .position(|entry| entry.path == selected_ref)
                .ok_or_else(|| anyhow!("Selected local audio file was not in its album queue"))?;
            return Ok(LocalPlaybackQueue {
                entries,
                start_index,
            });
        }

        let entries = self.resolve_to_files(reference)?;
        Ok(LocalPlaybackQueue {
            entries,
            start_index: 0,
        })
    }

    fn collect_audio_files(&self, folder: &Path, out: &mut Vec<LocalAudioEntry>) -> Result<()> {
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_audio_files(&path, out)?;
            } else if self.is_audio_file(&path) {
                out.push(self.file_entry(path)?);
            }
        }
        Ok(())
    }

    fn collect_direct_audio_files(
        &self,
        folder: &Path,
        out: &mut Vec<LocalAudioEntry>,
    ) -> Result<()> {
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && self.is_audio_file(&path) {
                out.push(self.file_entry(path)?);
            }
        }
        Ok(())
    }

    fn file_entry(&self, path: PathBuf) -> Result<LocalAudioEntry> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(strip_extension)
            .unwrap_or_else(|| path.display().to_string());
        self.file_entry_with_fallback(path, name)
    }

    fn file_entry_with_fallback(
        &self,
        path: PathBuf,
        fallback_name: String,
    ) -> Result<LocalAudioEntry> {
        let metadata = read_audio_metadata(&path);
        let name = metadata
            .as_ref()
            .and_then(|metadata| metadata.title.clone())
            .unwrap_or(fallback_name);
        let reference = self.path_to_ref(&path)?;
        Ok(LocalAudioEntry {
            id: reference.clone(),
            name,
            kind: LocalAudioEntryKind::File,
            path: reference,
            image_uri: None,
            metadata,
        })
    }

    pub fn resolve_artwork_ref(&self, reference: &str) -> Result<PathBuf> {
        let folder = self.resolve_folder_ref(reference)?;
        folder_artwork(&folder)
            .ok_or_else(|| anyhow!("Local audio artwork does not exist: {}", folder.display()))
    }

    fn resolve_folder_ref(&self, reference: &str) -> Result<PathBuf> {
        let path = self.resolve_path_ref(reference)?;
        if !path.is_dir() {
            anyhow::bail!("Local audio path is not a folder: {}", path.display());
        }
        Ok(path)
    }

    fn resolve_path_ref(&self, reference: &str) -> Result<PathBuf> {
        let (root_idx, relative) = parse_local_ref(reference)?;
        let root = self
            .roots
            .get(root_idx)
            .ok_or_else(|| anyhow!("Unknown local audio root: {root_idx}"))?;
        let root = root.canonicalize().unwrap_or_else(|_| root.clone());
        let path = if relative.is_empty() {
            root.clone()
        } else {
            root.join(relative)
        };
        let canonical = path.canonicalize().unwrap_or(path);

        if !canonical.starts_with(&root) {
            anyhow::bail!("Local audio path escapes configured root");
        }

        Ok(canonical)
    }

    fn path_to_ref(&self, path: &Path) -> Result<String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        for (idx, root) in self.roots.iter().enumerate() {
            let root = root.canonicalize().unwrap_or_else(|_| root.clone());
            if canonical.starts_with(&root) {
                let relative = canonical
                    .strip_prefix(&root)
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .trim_start_matches('/')
                    .to_string();
                if relative.is_empty() {
                    return Ok(format!("root:{idx}"));
                }
                return Ok(format!("root:{idx}/{relative}"));
            }
        }
        anyhow::bail!(
            "Path is outside configured local audio roots: {}",
            path.display()
        )
    }

    fn is_audio_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| self.allowed_extensions.contains(&ext.to_lowercase()))
            .unwrap_or(false)
    }

    fn folder_image_uri(&self, path: &Path) -> Option<String> {
        let reference = self.path_to_ref(path).ok()?;
        folder_artwork(path).map(|_| self.artwork_uri(&reference))
    }

    fn artwork_uri(&self, reference: &str) -> String {
        format!(
            "{}?path={}",
            self.artwork_path,
            utf8_percent_encode(reference, NON_ALPHANUMERIC)
        )
    }
}

impl LocalAudioPlayer {
    pub fn new(audio_sender: SyncSender<SinkEvent>) -> Self {
        let info = SpotifyPlayerInfo::stopped();
        let (info_sender, info_receiver) = watch::channel(info);
        Self {
            audio_sender,
            info_sender,
            info_receiver,
            state: Arc::new(Mutex::new(LocalPlaybackState::default())),
        }
    }

    pub fn player_info_channel(&self) -> watch::Receiver<SpotifyPlayerInfo> {
        self.info_receiver.clone()
    }

    pub fn play_queue(
        &self,
        queue: Vec<LocalAudioEntry>,
        start_index: usize,
        library: LocalAudioLibrary,
        source: impl Into<String>,
    ) -> Result<()> {
        self.play_queue_with_repeat(queue, start_index, library, source, true)
    }

    pub fn play_queue_with_repeat(
        &self,
        queue: Vec<LocalAudioEntry>,
        start_index: usize,
        library: LocalAudioLibrary,
        source: impl Into<String>,
        repeat: bool,
    ) -> Result<()> {
        if queue.is_empty() {
            self.stop();
            return Ok(());
        }
        self.start_from(queue, start_index, library, source.into(), repeat);
        Ok(())
    }

    pub fn play(&self, library: LocalAudioLibrary, source: impl Into<String>) {
        let (queue, index) = {
            let state = self.state.lock().unwrap();
            (state.queue.clone(), state.current_index)
        };
        if !queue.is_empty() {
            self.start_from(queue, index, library, source.into(), true);
        }
    }

    pub fn pause(&self) {
        self.cancel_current();
        let mut info = self.info_receiver.borrow().clone();
        info.status = SpotifyPlayerState::Paused;
        let _ = self.info_sender.send(info);
    }

    pub fn stop(&self) {
        self.cancel_current();
        let _ = self.info_sender.send(SpotifyPlayerInfo::stopped());
    }

    pub fn next(&self, library: LocalAudioLibrary, source: impl Into<String>) {
        let (queue, current_index) = {
            let state = self.state.lock().unwrap();
            (state.queue.clone(), state.current_index)
        };
        if queue.is_empty() {
            self.stop();
            return;
        }

        let next_index = (current_index + 1) % queue.len();
        self.start_from(queue, next_index, library, source.into(), true);
    }

    fn start_from(
        &self,
        queue: Vec<LocalAudioEntry>,
        index: usize,
        library: LocalAudioLibrary,
        source: String,
        repeat: bool,
    ) {
        self.cancel_current();
        if queue.is_empty() {
            let _ = self.info_sender.send(SpotifyPlayerInfo::stopped());
            return;
        }
        let index = index.min(queue.len() - 1);

        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut state = self.state.lock().unwrap();
            state.queue = queue.clone();
            state.current_index = index;
            state.cancel = Some(cancel.clone());
        }

        let audio_sender = self.audio_sender.clone();
        let info_sender = self.info_sender.clone();
        let state = self.state.clone();

        thread::spawn(move || {
            let mut current_index = index;
            let mut consecutive_failures = 0usize;

            loop {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }

                {
                    let mut state = state.lock().unwrap();
                    state.current_index = current_index;
                }

                let entry = queue[current_index].clone();
                let path = match library.resolve_ref(&entry.path) {
                    Ok(path) => path,
                    Err(e) => {
                        log::warn!("Skipping local audio item {}: {e}", entry.path);
                        continue;
                    }
                };

                let artist = entry
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.display_artist())
                    .unwrap_or_else(|| "Local files".to_string());
                let artwork_url = path
                    .parent()
                    .and_then(|folder| library.folder_image_uri(folder))
                    .unwrap_or_default();

                let _ = info_sender.send(SpotifyPlayerInfo {
                    status: SpotifyPlayerState::Playing,
                    shuffle: false,
                    metadata: Some(MusicMetadata {
                        artist,
                        title: entry.name.clone(),
                        artwork_url,
                        source: Some(source.clone()),
                    }),
                    sleep_timer: None,
                });

                let volume = playback_volume_for_source(&source);
                if let Err(e) =
                    play_file_blocking(&path, audio_sender.clone(), cancel.clone(), volume)
                {
                    log::warn!("Failed to play local audio file {}: {e}", path.display());
                    consecutive_failures += 1;
                } else {
                    consecutive_failures = 0;
                }

                if consecutive_failures >= queue.len() {
                    log::warn!("Stopping local playback after every queued file failed");
                    break;
                }

                if current_index + 1 >= queue.len() {
                    if repeat {
                        current_index = 0;
                    } else {
                        break;
                    }
                } else {
                    current_index += 1;
                }
            }

            if !cancel.load(Ordering::Relaxed) {
                let _ = info_sender.send(SpotifyPlayerInfo::stopped());
            }
        });
    }

    fn cancel_current(&self) {
        let cancel = {
            let mut state = self.state.lock().unwrap();
            state.cancel.take()
        };
        if let Some(cancel) = cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn play_file_blocking(
    path: &Path,
    audio_sender: SyncSender<SinkEvent>,
    cancel: Arc<AtomicBool>,
    volume: f64,
) -> Result<()> {
    let uri = gst::glib::filename_to_uri(path, None)
        .map_err(|_| anyhow!("Failed to build file URI for {}", path.display()))?;
    let pipeline_description = format!(
        "uridecodebin uri={} ! audioconvert ! volume volume={} ! audioresample ! audio/x-raw,format=F64LE,channels=2,rate=44100,layout=interleaved ! appsink name=local_audio_sink sync=true",
        uri.as_str(),
        volume
    );

    let element = gst::parse::launch(&pipeline_description).with_context(|| {
        format!(
            "Failed to create local audio pipeline for {}",
            path.display()
        )
    })?;
    let pipeline = element
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| anyhow!("Local audio GStreamer description did not create a pipeline"))?;
    let appsink = pipeline
        .by_name("local_audio_sink")
        .ok_or_else(|| anyhow!("Local audio pipeline has no appsink"))?
        .dynamic_cast::<AppSink>()
        .map_err(|_| anyhow!("local_audio_sink is not an AppSink"))?;

    audio_sender.send(SinkEvent::Start)?;
    pipeline.set_state(gst::State::Playing)?;

    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        if let Some(sample) = appsink.try_pull_sample(gst::ClockTime::from_mseconds(100)) {
            let buffer = sample
                .buffer()
                .ok_or_else(|| anyhow!("Local audio sample has no buffer"))?;
            let map = buffer.map_readable()?;
            let bytes = map.as_slice();
            if bytes.is_empty() {
                continue;
            }
            if bytes.len() % std::mem::size_of::<f64>() != 0 {
                log::warn!("Ignoring unaligned local audio buffer");
                continue;
            }

            let sample_count = bytes.len() / std::mem::size_of::<f64>();
            let samples =
                unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const f64, sample_count) }
                    .to_vec();
            audio_sender.send(SinkEvent::Packet(samples))?;
            continue;
        }

        if appsink.is_eos() {
            break;
        }
    }

    pipeline.set_state(gst::State::Null)?;
    let _ = audio_sender.send(SinkEvent::Stop);
    Ok(())
}

fn playback_volume_for_source(source: &str) -> f64 {
    if source == "local" {
        LOCAL_FILE_PLAYBACK_VOLUME
    } else {
        1.0
    }
}

fn parse_local_ref(reference: &str) -> Result<(usize, String)> {
    let reference = reference
        .strip_prefix("local:file:")
        .or_else(|| reference.strip_prefix("local:folder:"))
        .or_else(|| reference.strip_prefix("youtube:file:"))
        .or_else(|| reference.strip_prefix("youtube:folder:"))
        .unwrap_or(reference);
    let rest = reference
        .strip_prefix("root:")
        .ok_or_else(|| anyhow!("Invalid local audio reference: {reference}"))?;
    let mut parts = rest.splitn(2, '/');
    let root_idx = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid local audio reference: {reference}"))?
        .parse::<usize>()?;
    let relative = parts.next().unwrap_or_default().to_string();
    Ok((root_idx, relative))
}

fn strip_extension(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name)
        .to_string()
}

const FOLDER_ARTWORK_NAMES: &[&str] = &[
    "coverart.jpg",
    "coverart.jpeg",
    "coverart.png",
    "coverart.webp",
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "cover.webp",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "folder.webp",
    "front.jpg",
    "front.jpeg",
    "front.png",
    "front.webp",
];

fn folder_artwork(folder: &Path) -> Option<PathBuf> {
    for name in FOLDER_ARTWORK_NAMES {
        let path = folder.join(name);
        if path.is_file() {
            return Some(path);
        }
    }

    fs::read_dir(folder).ok()?.find_map(|entry| {
        let path = entry.ok()?.path();
        if path.is_file() && is_folder_artwork_name(&path) {
            Some(path)
        } else {
            None
        }
    })
}

fn is_folder_artwork_name(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());
    name.as_deref()
        .is_some_and(|name| FOLDER_ARTWORK_NAMES.contains(&name))
}

fn read_audio_metadata(path: &Path) -> Option<LocalAudioMetadata> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }

    let metadata_opts = MetadataOptions::default();
    let format_opts = FormatOptions::default();
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .ok()?;

    let mut metadata = LocalAudioMetadata::default();

    if let Some(mut probed_metadata) = probed.metadata.get() {
        if let Some(revision) = probed_metadata.skip_to_latest() {
            metadata.apply_revision(revision);
        }
    }

    if let Some(revision) = probed.format.metadata().skip_to_latest() {
        metadata.apply_revision(revision);
    }

    if metadata.is_empty() {
        None
    } else {
        Some(metadata)
    }
}

impl LocalAudioMetadata {
    fn apply_revision(&mut self, revision: &MetadataRevision) {
        for tag in revision.tags() {
            let value = tag.value.to_string();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }

            match tag.std_key {
                Some(StandardTagKey::TrackTitle) => self.set_missing_title(value),
                Some(StandardTagKey::Artist) => self.set_missing_artist(value),
                Some(StandardTagKey::Album) => self.set_missing_album(value),
                Some(StandardTagKey::AlbumArtist) => self.set_missing_album_artist(value),
                Some(StandardTagKey::TrackNumber) => self.set_missing_track_number(value),
                Some(StandardTagKey::DiscNumber) => self.set_missing_disc_number(value),
                Some(StandardTagKey::Date) | Some(StandardTagKey::ReleaseDate) => {
                    self.set_missing_date(value)
                }
                Some(StandardTagKey::Genre) => self.set_missing_genre(value),
                _ => self.apply_legacy_key(&tag.key, value),
            }
        }
    }

    fn apply_legacy_key(&mut self, key: &str, value: &str) {
        match key.to_ascii_lowercase().as_str() {
            "title" | "tit2" => self.set_missing_title(value),
            "artist" | "tpe1" => self.set_missing_artist(value),
            "album" | "talb" => self.set_missing_album(value),
            "albumartist" | "album_artist" | "tpe2" => self.set_missing_album_artist(value),
            "tracknumber" | "track_number" | "track" | "trck" => {
                self.set_missing_track_number(value)
            }
            "discnumber" | "disc_number" | "disc" | "tpos" => self.set_missing_disc_number(value),
            "date" | "year" | "tdrc" | "tyer" => self.set_missing_date(value),
            "genre" | "tcon" => self.set_missing_genre(value),
            _ => {}
        }
    }

    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.artist.is_none()
            && self.album.is_none()
            && self.album_artist.is_none()
            && self.track_number.is_none()
            && self.disc_number.is_none()
            && self.date.is_none()
            && self.genre.is_none()
    }

    fn set_missing_title(&mut self, value: &str) {
        set_missing(&mut self.title, value);
    }

    fn set_missing_artist(&mut self, value: &str) {
        set_missing(&mut self.artist, value);
    }

    fn set_missing_album(&mut self, value: &str) {
        set_missing(&mut self.album, value);
    }

    fn set_missing_album_artist(&mut self, value: &str) {
        set_missing(&mut self.album_artist, value);
    }

    fn set_missing_track_number(&mut self, value: &str) {
        set_missing(&mut self.track_number, value);
    }

    fn set_missing_disc_number(&mut self, value: &str) {
        set_missing(&mut self.disc_number, value);
    }

    fn set_missing_date(&mut self, value: &str) {
        set_missing(&mut self.date, value);
    }

    fn set_missing_genre(&mut self, value: &str) {
        set_missing(&mut self.genre, value);
    }

    fn display_artist(&self) -> Option<String> {
        self.artist
            .clone()
            .or_else(|| self.album_artist.clone())
            .or_else(|| self.album.clone())
    }
}

fn set_missing(target: &mut Option<String>, value: &str) {
    if target.is_none() {
        *target = Some(value.to_string());
    }
}

fn compare_local_entries(a: &LocalAudioEntry, b: &LocalAudioEntry) -> CmpOrdering {
    a.kind
        .cmp_rank()
        .cmp(&b.kind.cmp_rank())
        .then_with(|| compare_track_position(a, b))
        .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
}

fn compare_track_position(a: &LocalAudioEntry, b: &LocalAudioEntry) -> CmpOrdering {
    match (track_position(a), track_position(b)) {
        (Some(a_position), Some(b_position)) => a_position.cmp(&b_position),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        (None, None) => CmpOrdering::Equal,
    }
}

fn track_position(entry: &LocalAudioEntry) -> Option<(u32, u32)> {
    let metadata = entry.metadata.as_ref()?;
    let track = parse_metadata_number(metadata.track_number.as_deref())?;
    let disc = parse_metadata_number(metadata.disc_number.as_deref()).unwrap_or(1);
    Some((disc, track))
}

fn parse_metadata_number(value: Option<&str>) -> Option<u32> {
    let number = value?
        .trim()
        .split(['/', '-'])
        .next()
        .unwrap_or_default()
        .trim()
        .parse::<u32>()
        .ok()?;
    (number > 0).then_some(number)
}

trait LocalEntrySort {
    fn cmp_rank(&self) -> u8;
}

impl LocalEntrySort for LocalAudioEntryKind {
    fn cmp_rank(&self) -> u8 {
        match self {
            LocalAudioEntryKind::Folder => 0,
            LocalAudioEntryKind::File => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_entry(path: &str, name: &str, track: Option<&str>) -> LocalAudioEntry {
        LocalAudioEntry {
            id: path.to_string(),
            name: name.to_string(),
            kind: LocalAudioEntryKind::File,
            path: path.to_string(),
            image_uri: None,
            metadata: track.map(|track_number| LocalAudioMetadata {
                track_number: Some(track_number.to_string()),
                ..LocalAudioMetadata::default()
            }),
        }
    }

    #[test]
    fn sorts_files_by_track_number_before_display_title() {
        let mut entries = vec![
            file_entry("root:0/10 - Alleluia.ogg", "Alleluia", Some("10")),
            file_entry("root:0/02 - Ave Maria.ogg", "Ave Maria", Some("2")),
            file_entry("root:0/01 - Gloria.ogg", "Gloria", Some("1")),
        ];

        entries.sort_by(compare_local_entries);

        let names = entries
            .into_iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["Gloria", "Ave Maria", "Alleluia"]);
    }

    #[test]
    fn sorts_files_by_disc_then_track_number() {
        let mut entries = vec![
            file_entry_with_disc("root:0/disc2-track1.ogg", "Disc 2 Track 1", "2", "1"),
            file_entry_with_disc("root:0/disc1-track2.ogg", "Disc 1 Track 2", "1", "2"),
            file_entry_with_disc("root:0/disc1-track1.ogg", "Disc 1 Track 1", "1", "1"),
        ];

        entries.sort_by(compare_local_entries);

        let names = entries
            .into_iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            ["Disc 1 Track 1", "Disc 1 Track 2", "Disc 2 Track 1"]
        );
    }

    #[test]
    fn parses_fractional_track_numbers() {
        let mut entries = vec![
            file_entry("root:0/10.ogg", "Track 10", Some("10/12")),
            file_entry("root:0/02.ogg", "Track 2", Some("2/12")),
        ];

        entries.sort_by(compare_local_entries);

        let names = entries
            .into_iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["Track 2", "Track 10"]);
    }

    #[test]
    fn parses_youtube_file_refs() {
        let (root, relative) = parse_local_ref("youtube:file:root:0/Channel - Title.webm").unwrap();

        assert_eq!(root, 0);
        assert_eq!(relative, "Channel - Title.webm");
    }

    #[test]
    fn local_source_playback_uses_reduced_volume() {
        assert_eq!(playback_volume_for_source("local"), 0.35);
        assert_eq!(playback_volume_for_source("youtube"), 1.0);
    }

    #[test]
    fn local_root_lists_album_folders_instead_of_music_root() {
        let root = std::env::temp_dir().join(format!(
            "carechords-local-audio-albums-test-{}",
            std::process::id()
        ));
        let first_album = root.join("Artist One").join("2024 - First Album");
        let second_album = root.join("Artist Two").join("2025 - Second Album");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&first_album).unwrap();
        fs::create_dir_all(&second_album).unwrap();
        File::create(first_album.join("01 - First.ogg")).unwrap();
        File::create(second_album.join("01 - Second.ogg")).unwrap();

        let library = LocalAudioLibrary::new(&LocalAudioSettings {
            roots: vec![root.to_string_lossy().to_string()],
            allowed_extensions: vec!["ogg".to_string()],
        });

        let entries = library.list(None).unwrap();
        let names = entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, ["2024 - First Album", "2025 - Second Album"]);
        assert!(
            entries
                .iter()
                .all(|entry| entry.kind == LocalAudioEntryKind::Folder)
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn selected_file_resolves_to_album_queue_starting_at_that_file() {
        let root = std::env::temp_dir().join(format!(
            "carechords-local-audio-queue-test-{}",
            std::process::id()
        ));
        let album = root.join("Artist").join("Album");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&album).unwrap();
        File::create(album.join("01 - First.ogg")).unwrap();
        File::create(album.join("02 - Second.ogg")).unwrap();
        File::create(album.join("03 - Third.ogg")).unwrap();
        File::create(album.join("coverart.jpg")).unwrap();

        let library = LocalAudioLibrary::new(&LocalAudioSettings {
            roots: vec![root.to_string_lossy().to_string()],
            allowed_extensions: vec!["ogg".to_string()],
        });

        let queue = library
            .resolve_playback_queue("root:0/Artist/Album/02 - Second.ogg")
            .unwrap();

        let names = queue
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["01 - First", "02 - Second", "03 - Third"]);
        assert_eq!(queue.start_index, 1);

        fs::remove_dir_all(&root).unwrap();
    }

    fn file_entry_with_disc(
        path: &str,
        name: &str,
        disc_number: &str,
        track_number: &str,
    ) -> LocalAudioEntry {
        LocalAudioEntry {
            id: path.to_string(),
            name: name.to_string(),
            kind: LocalAudioEntryKind::File,
            path: path.to_string(),
            image_uri: None,
            metadata: Some(LocalAudioMetadata {
                disc_number: Some(disc_number.to_string()),
                track_number: Some(track_number.to_string()),
                ..LocalAudioMetadata::default()
            }),
        }
    }
}
