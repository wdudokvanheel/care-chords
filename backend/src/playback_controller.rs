use crate::local_audio::{LocalAudioLibrary, LocalAudioPlayer};
use crate::spotify_client::{PlaylistSummary, SpotifyClient};
use crate::spotify_player::{PlayerCommand, SpotifyPlayerInfo, SpotifyPlayerState};
use crate::system_playlists::{SystemPlaylistItem, SystemPlaylistStore};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;
use tokio::sync::watch;

#[derive(Clone)]
pub struct PlaybackController {
    spotify: Arc<Mutex<SpotifySlot>>,
    local_library: LocalAudioLibrary,
    youtube_library: LocalAudioLibrary,
    local_player: Arc<LocalAudioPlayer>,
    playlists: SystemPlaylistStore,
    system_queue: Arc<Mutex<SystemQueue>>,
    active_source: Arc<Mutex<ActiveSource>>,
    info_sender: watch::Sender<SpotifyPlayerInfo>,
    info_receiver: watch::Receiver<SpotifyPlayerInfo>,
}

#[derive(Clone)]
struct SpotifyRuntime {
    client: Arc<SpotifyClient>,
    commands: Sender<PlayerCommand>,
}

enum SpotifySlot {
    AuthPending,
    Ready(SpotifyRuntime),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveSource {
    None,
    Spotify,
    Local,
    Youtube,
}

#[derive(Default)]
struct SystemQueue {
    items: Vec<SystemPlaylistItem>,
    current_index: usize,
    collection_items: Vec<SystemPlaylistItem>,
    collection_index: usize,
    collection_owner_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemQueueState {
    pub items: Vec<SystemPlaylistItem>,
    pub current_index: Option<usize>,
    pub repeat_last: bool,
}

#[derive(Debug, Serialize)]
pub struct AudioSourceStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

#[derive(Debug, Deserialize)]
pub struct PlayRefRequest {
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Deserialize)]
pub struct QueueItemRequest {
    pub source: String,
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct ReorderQueueRequest {
    pub from_index: usize,
    pub to_index: usize,
}

#[derive(Debug, Deserialize)]
pub struct LegacyPlaylistRequest {
    pub uri: String,
}

impl PlaybackController {
    pub fn new(
        local_library: LocalAudioLibrary,
        youtube_library: LocalAudioLibrary,
        local_player: Arc<LocalAudioPlayer>,
        playlists: SystemPlaylistStore,
    ) -> Self {
        let (info_sender, info_receiver) = watch::channel(SpotifyPlayerInfo::stopped());

        let controller = Self {
            spotify: Arc::new(Mutex::new(SpotifySlot::AuthPending)),
            local_library,
            youtube_library,
            local_player,
            playlists,
            system_queue: Arc::new(Mutex::new(SystemQueue::default())),
            active_source: Arc::new(Mutex::new(ActiveSource::None)),
            info_sender,
            info_receiver,
        };
        controller.spawn_status_forwarders();
        controller
    }

    pub fn attach_spotify(&self, spotify: Arc<SpotifyClient>) {
        let runtime = SpotifyRuntime {
            commands: spotify.player_command_channel(),
            client: spotify,
        };

        {
            let mut slot = self.spotify.lock().unwrap();
            *slot = SpotifySlot::Ready(runtime.clone());
        }

        self.spawn_spotify_status_forwarder(runtime.client.player_info_channel());
    }

    pub fn sources(&self) -> Vec<AudioSourceStatus> {
        let spotify_available = matches!(*self.spotify.lock().unwrap(), SpotifySlot::Ready(_));
        vec![
            AudioSourceStatus {
                id: "local",
                name: "Local files",
                available: true,
                reason: None,
            },
            AudioSourceStatus {
                id: "youtube",
                name: "YouTube",
                available: true,
                reason: None,
            },
            AudioSourceStatus {
                id: "spotify",
                name: "Spotify",
                available: spotify_available,
                reason: if spotify_available {
                    None
                } else {
                    Some("auth_pending")
                },
            },
        ]
    }

    pub fn local_library(&self) -> LocalAudioLibrary {
        self.local_library.clone()
    }

    pub fn youtube_library(&self) -> LocalAudioLibrary {
        self.youtube_library.clone()
    }

    pub fn system_playlists(&self) -> SystemPlaylistStore {
        self.playlists.clone()
    }

    pub async fn spotify_playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let spotify = self.spotify_client()?;
        spotify.playlists().await
    }

    pub fn info_channel(&self) -> watch::Receiver<SpotifyPlayerInfo> {
        self.info_receiver.clone()
    }

    pub fn current_info(&self) -> SpotifyPlayerInfo {
        self.info_receiver.borrow().clone()
    }

    pub async fn play_ref(&self, reference: &str) -> Result<()> {
        if let Some(id) = reference.strip_prefix("system:playlist:") {
            return self.play_system_playlist(id).await;
        }

        self.clear_system_queue();
        self.play_ref_without_system(reference).await
    }

    async fn play_ref_without_system(&self, reference: &str) -> Result<()> {
        if reference.starts_with("local:file:")
            || reference.starts_with("local:folder:")
            || reference.starts_with("root:")
        {
            self.play_local_ref(
                reference,
                self.local_library.clone(),
                ActiveSource::Local,
                "local",
            )?;
            return Ok(());
        }

        if reference.starts_with("youtube:file:") || reference.starts_with("youtube:folder:") {
            self.play_local_ref(
                reference,
                self.youtube_library.clone(),
                ActiveSource::Youtube,
                "youtube",
            )?;
            return Ok(());
        }

        if reference.starts_with("spotify:") {
            self.play_spotify_ref(reference).await?;
            return Ok(());
        }

        anyhow::bail!("Unsupported audio reference: {reference}");
    }

    pub async fn play_system_playlist(&self, id: &str) -> Result<()> {
        let playlist = self
            .playlists
            .get(id)
            .ok_or_else(|| anyhow!("Unknown system playlist: {id}"))?;

        if playlist.items.is_empty() {
            anyhow::bail!("System playlist is empty");
        }

        {
            let mut system_queue = self.system_queue.lock().unwrap();
            system_queue.items = playlist.items;
            system_queue.current_index = 0;
            system_queue.collection_items.clear();
            system_queue.collection_index = 0;
            system_queue.collection_owner_id = None;
        }
        self.play_current_system_item().await
    }

    pub fn queue_state(&self) -> SystemQueueState {
        let system_queue = self.system_queue.lock().unwrap();
        SystemQueueState {
            items: system_queue.items.clone(),
            current_index: if system_queue.items.is_empty() {
                None
            } else {
                Some(system_queue.current_index.min(system_queue.items.len() - 1))
            },
            repeat_last: true,
        }
    }

    pub async fn enqueue(&self, req: QueueItemRequest) -> Result<SystemQueueState> {
        let should_start = {
            let mut system_queue = self.system_queue.lock().unwrap();
            let should_start = system_queue.items.is_empty();
            system_queue.items.push(SystemPlaylistItem {
                id: format!("queue-{}-{}", now_epoch_secs(), rand::random::<u32>()),
                source: req.source,
                kind: req.kind,
                reference: req.reference,
                title: req.title,
            });
            if should_start {
                system_queue.current_index = 0;
                system_queue.collection_items.clear();
                system_queue.collection_index = 0;
                system_queue.collection_owner_id = None;
            }
            should_start
        };

        if should_start {
            self.play_current_system_item().await?;
        }

        Ok(self.queue_state())
    }

    pub async fn remove_queue_item(&self, item_id: &str) -> Result<SystemQueueState> {
        let action = {
            let mut system_queue = self.system_queue.lock().unwrap();
            let index = system_queue
                .items
                .iter()
                .position(|item| item.id == item_id)
                .ok_or_else(|| anyhow!("Unknown queue item: {item_id}"))?;
            let removed_id = system_queue.items[index].id.clone();
            let was_current = index == system_queue.current_index;
            system_queue.items.remove(index);
            if was_current || system_queue.collection_owner_id.as_ref() == Some(&removed_id) {
                system_queue.collection_items.clear();
                system_queue.collection_index = 0;
                system_queue.collection_owner_id = None;
            }

            if system_queue.items.is_empty() {
                system_queue.current_index = 0;
                QueueMutationAction::Stop
            } else {
                if index < system_queue.current_index {
                    system_queue.current_index -= 1;
                } else if system_queue.current_index >= system_queue.items.len() {
                    system_queue.current_index = system_queue.items.len() - 1;
                }

                if was_current {
                    QueueMutationAction::Restart
                } else {
                    QueueMutationAction::None
                }
            }
        };

        self.apply_queue_mutation_action(action).await?;
        Ok(self.queue_state())
    }

    pub async fn clear_queue(&self) -> Result<SystemQueueState> {
        {
            let mut system_queue = self.system_queue.lock().unwrap();
            system_queue.items.clear();
            system_queue.current_index = 0;
            system_queue.collection_items.clear();
            system_queue.collection_index = 0;
            system_queue.collection_owner_id = None;
        }
        self.stop_active().await?;
        Ok(self.queue_state())
    }

    pub async fn reorder_queue(&self, req: ReorderQueueRequest) -> Result<SystemQueueState> {
        let action = {
            let mut system_queue = self.system_queue.lock().unwrap();
            if req.from_index >= system_queue.items.len()
                || req.to_index >= system_queue.items.len()
            {
                anyhow::bail!("Queue move index out of bounds");
            }
            if req.from_index == req.to_index {
                return Ok(self.queue_state());
            }

            let current = system_queue.current_index;
            let item = system_queue.items.remove(req.from_index);
            system_queue.items.insert(req.to_index, item);
            system_queue.current_index = moved_current_index(current, req.from_index, req.to_index);
            QueueMutationAction::None
        };

        self.apply_queue_mutation_action(action).await?;
        Ok(self.queue_state())
    }

    pub async fn play_queue_index(&self, index: usize) -> Result<SystemQueueState> {
        {
            let mut system_queue = self.system_queue.lock().unwrap();
            if index >= system_queue.items.len() {
                anyhow::bail!("Queue index out of bounds");
            }
            system_queue.current_index = index;
            system_queue.collection_items.clear();
            system_queue.collection_index = 0;
            system_queue.collection_owner_id = None;
        }
        self.play_current_system_item().await?;
        Ok(self.queue_state())
    }

    pub async fn play(&self) -> Result<()> {
        match self.active() {
            ActiveSource::Spotify => self.send_spotify(PlayerCommand::Play).await,
            ActiveSource::Local => {
                self.local_player.play(self.local_library.clone(), "local");
                Ok(())
            }
            ActiveSource::Youtube => {
                self.local_player
                    .play(self.youtube_library.clone(), "youtube");
                Ok(())
            }
            ActiveSource::None => Ok(()),
        }
    }

    pub async fn pause(&self) -> Result<()> {
        match self.active() {
            ActiveSource::Spotify => self.send_spotify(PlayerCommand::Pause).await,
            ActiveSource::Local | ActiveSource::Youtube => {
                self.local_player.pause();
                Ok(())
            }
            ActiveSource::None => Ok(()),
        }
    }

    pub async fn next(&self) -> Result<()> {
        if self.advance_system_queue().await? {
            return Ok(());
        }

        match self.active() {
            ActiveSource::Spotify => self.send_spotify(PlayerCommand::Next).await,
            ActiveSource::Local => {
                self.local_player.next(self.local_library.clone(), "local");
                Ok(())
            }
            ActiveSource::Youtube => {
                self.local_player
                    .next(self.youtube_library.clone(), "youtube");
                Ok(())
            }
            ActiveSource::None => Ok(()),
        }
    }

    pub async fn sleep(&self, seconds: u32) -> Result<()> {
        match self.active() {
            ActiveSource::Spotify => self.send_spotify(PlayerCommand::Sleep(seconds)).await,
            ActiveSource::Local | ActiveSource::Youtube | ActiveSource::None => Ok(()),
        }
    }

    pub async fn shuffle(&self, shuffle: bool) -> Result<SpotifyPlayerInfo> {
        if self.active() == ActiveSource::Spotify {
            self.send_spotify(PlayerCommand::Shuffle(shuffle)).await?;
        }
        Ok(self.current_info())
    }

    fn play_local_ref(
        &self,
        reference: &str,
        library: LocalAudioLibrary,
        active_source: ActiveSource,
        source_name: &str,
    ) -> Result<()> {
        if self.active() == ActiveSource::Spotify {
            if let Ok(commands) = self.spotify_commands() {
                tokio::spawn(async move {
                    let _ = commands.send(PlayerCommand::Pause).await;
                });
            }
        }
        self.set_active(active_source);
        let queue = library.resolve_playback_queue(reference)?;
        self.local_player.play_queue(
            queue.entries,
            queue.start_index,
            library,
            source_name.to_string(),
        )?;
        Ok(())
    }

    async fn play_spotify_ref(&self, reference: &str) -> Result<()> {
        self.local_player.stop();
        self.set_active(ActiveSource::Spotify);
        self.send_spotify(PlayerCommand::PlayRef {
            uri: reference.to_string(),
            repeat: true,
        })
        .await
    }

    async fn play_spotify_queue_ref(&self, reference: &str) -> Result<()> {
        self.local_player.stop();
        self.set_active(ActiveSource::Spotify);
        self.send_spotify(PlayerCommand::PlayRef {
            uri: reference.to_string(),
            repeat: false,
        })
        .await
    }

    async fn send_spotify(&self, command: PlayerCommand) -> Result<()> {
        let commands = self.spotify_commands()?;
        commands.send(command).await?;
        Ok(())
    }

    fn spotify_client(&self) -> Result<Arc<SpotifyClient>> {
        match &*self.spotify.lock().unwrap() {
            SpotifySlot::Ready(runtime) => Ok(runtime.client.clone()),
            SpotifySlot::AuthPending => anyhow::bail!("Spotify authentication is still pending"),
        }
    }

    fn spotify_commands(&self) -> Result<Sender<PlayerCommand>> {
        match &*self.spotify.lock().unwrap() {
            SpotifySlot::Ready(runtime) => Ok(runtime.commands.clone()),
            SpotifySlot::AuthPending => anyhow::bail!("Spotify authentication is still pending"),
        }
    }

    fn active(&self) -> ActiveSource {
        *self.active_source.lock().unwrap()
    }

    fn set_active(&self, source: ActiveSource) {
        *self.active_source.lock().unwrap() = source;
    }

    fn clear_system_queue(&self) {
        let mut system_queue = self.system_queue.lock().unwrap();
        system_queue.items.clear();
        system_queue.current_index = 0;
        system_queue.collection_items.clear();
        system_queue.collection_index = 0;
        system_queue.collection_owner_id = None;
    }

    fn spawn_status_forwarders(&self) {
        let mut local_status = self.local_player.player_info_channel();
        let local_active = self.active_source.clone();
        let local_sender = self.info_sender.clone();
        let local_controller = self.clone();
        tokio::spawn(async move {
            while local_status.changed().await.is_ok() {
                if matches!(
                    *local_active.lock().unwrap(),
                    ActiveSource::Local | ActiveSource::Youtube
                ) {
                    let status = local_status.borrow().clone();
                    let _ = local_sender.send(status.clone());
                    if status.status == SpotifyPlayerState::Stopped {
                        let _ = local_controller.advance_system_queue().await;
                    }
                }
            }
        });
    }

    fn spawn_spotify_status_forwarder(
        &self,
        mut spotify_status: watch::Receiver<SpotifyPlayerInfo>,
    ) {
        let spotify_active = self.active_source.clone();
        let spotify_sender = self.info_sender.clone();
        let spotify_controller = self.clone();
        tokio::spawn(async move {
            while spotify_status.changed().await.is_ok() {
                if *spotify_active.lock().unwrap() == ActiveSource::Spotify {
                    let status = spotify_status.borrow().clone();
                    let _ = spotify_sender.send(status.clone());
                    if status.status == SpotifyPlayerState::Stopped {
                        let _ = spotify_controller.advance_system_queue().await;
                    }
                }
            }
        });
    }

    async fn advance_system_queue(&self) -> Result<bool> {
        let should_play = {
            let mut system_queue = self.system_queue.lock().unwrap();
            if system_queue.items.is_empty() {
                false
            } else if system_queue.collection_owner_id.is_some()
                && system_queue.collection_index + 1 < system_queue.collection_items.len()
            {
                system_queue.collection_index += 1;
                true
            } else {
                system_queue.collection_items.clear();
                system_queue.collection_index = 0;
                system_queue.collection_owner_id = None;
                if system_queue.current_index + 1 < system_queue.items.len() {
                    system_queue.current_index += 1;
                }
                true
            }
        };

        if should_play {
            self.play_current_system_item().await?;
        }

        Ok(should_play)
    }

    async fn play_current_system_item(&self) -> Result<()> {
        let item = {
            let system_queue = self.system_queue.lock().unwrap();
            system_queue
                .items
                .get(system_queue.current_index)
                .cloned()
                .ok_or_else(|| anyhow!("System playlist queue is empty"))?
        };

        if let Some(id) = item.reference.strip_prefix("system:playlist:") {
            let playlist = self
                .playlists
                .get(id)
                .ok_or_else(|| anyhow!("Unknown system playlist: {id}"))?;
            if playlist.items.is_empty() {
                anyhow::bail!("System playlist is empty");
            }
            let mut system_queue = self.system_queue.lock().unwrap();
            system_queue.collection_items = playlist.items;
            system_queue.collection_index = 0;
            system_queue.collection_owner_id = Some(item.id);
        }

        self.play_current_system_leaf().await
    }

    async fn play_current_system_leaf(&self) -> Result<()> {
        let item = {
            let system_queue = self.system_queue.lock().unwrap();
            if let Some(item) = system_queue
                .collection_items
                .get(system_queue.collection_index)
            {
                item.clone()
            } else {
                system_queue
                    .items
                    .get(system_queue.current_index)
                    .cloned()
                    .ok_or_else(|| anyhow!("System queue is empty"))?
            }
        };
        self.play_queue_leaf(&item).await
    }

    async fn play_queue_leaf(&self, item: &SystemPlaylistItem) -> Result<()> {
        if item.reference.starts_with("system:playlist:") {
            anyhow::bail!("Nested system playlists are not supported in the queue");
        }

        if item.reference.starts_with("local:file:")
            || item.reference.starts_with("local:folder:")
            || item.reference.starts_with("root:")
        {
            self.play_local_queue_ref(
                &item.reference,
                self.local_library.clone(),
                ActiveSource::Local,
                "local",
            )?;
            return Ok(());
        }

        if item.reference.starts_with("youtube:file:")
            || item.reference.starts_with("youtube:folder:")
        {
            self.play_local_queue_ref(
                &item.reference,
                self.youtube_library.clone(),
                ActiveSource::Youtube,
                "youtube",
            )?;
            return Ok(());
        }

        if item.reference.starts_with("spotify:") {
            self.play_spotify_queue_ref(&item.reference).await?;
            return Ok(());
        }

        anyhow::bail!("Unsupported audio reference: {}", item.reference);
    }

    fn play_local_queue_ref(
        &self,
        reference: &str,
        library: LocalAudioLibrary,
        active_source: ActiveSource,
        source_name: &str,
    ) -> Result<()> {
        if self.active() == ActiveSource::Spotify {
            if let Ok(commands) = self.spotify_commands() {
                tokio::spawn(async move {
                    let _ = commands.send(PlayerCommand::Pause).await;
                });
            }
        }
        self.set_active(active_source);
        let entries = library.resolve_to_files(reference)?;
        self.local_player.play_queue_with_repeat(
            entries,
            0,
            library,
            source_name.to_string(),
            false,
        )?;
        Ok(())
    }

    async fn stop_active(&self) -> Result<()> {
        match self.active() {
            ActiveSource::Spotify => {
                let _ = self.send_spotify(PlayerCommand::Pause).await;
            }
            ActiveSource::Local | ActiveSource::Youtube => {
                self.local_player.stop();
            }
            ActiveSource::None => {}
        }
        self.set_active(ActiveSource::None);
        let _ = self.info_sender.send(SpotifyPlayerInfo::stopped());
        Ok(())
    }

    async fn apply_queue_mutation_action(&self, action: QueueMutationAction) -> Result<()> {
        match action {
            QueueMutationAction::None => Ok(()),
            QueueMutationAction::Restart => self.play_current_system_item().await,
            QueueMutationAction::Stop => self.stop_active().await,
        }
    }
}

enum QueueMutationAction {
    None,
    Restart,
    Stop,
}

fn moved_current_index(current: usize, from: usize, to: usize) -> usize {
    if current == from {
        to
    } else if from < current && to >= current {
        current - 1
    } else if from > current && to <= current {
        current + 1
    } else {
        current
    }
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
