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
}

#[derive(Default)]
struct SystemQueue {
    items: Vec<SystemPlaylistItem>,
    current_index: usize,
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
pub struct LegacyPlaylistRequest {
    pub uri: String,
}

impl PlaybackController {
    pub fn new(
        local_library: LocalAudioLibrary,
        local_player: Arc<LocalAudioPlayer>,
        playlists: SystemPlaylistStore,
    ) -> Self {
        let (info_sender, info_receiver) = watch::channel(SpotifyPlayerInfo::stopped());

        let controller = Self {
            spotify: Arc::new(Mutex::new(SpotifySlot::AuthPending)),
            local_library,
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
            self.play_local_ref(reference)?;
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
        }
        self.play_current_system_item().await
    }

    pub async fn play(&self) -> Result<()> {
        match self.active() {
            ActiveSource::Spotify => self.send_spotify(PlayerCommand::Play).await,
            ActiveSource::Local => {
                self.local_player.play(self.local_library.clone());
                Ok(())
            }
            ActiveSource::None => Ok(()),
        }
    }

    pub async fn pause(&self) -> Result<()> {
        match self.active() {
            ActiveSource::Spotify => self.send_spotify(PlayerCommand::Pause).await,
            ActiveSource::Local => {
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
                self.local_player.next(self.local_library.clone());
                Ok(())
            }
            ActiveSource::None => Ok(()),
        }
    }

    pub async fn sleep(&self, seconds: u32) -> Result<()> {
        match self.active() {
            ActiveSource::Spotify => self.send_spotify(PlayerCommand::Sleep(seconds)).await,
            ActiveSource::Local | ActiveSource::None => Ok(()),
        }
    }

    pub async fn shuffle(&self, shuffle: bool) -> Result<SpotifyPlayerInfo> {
        if self.active() == ActiveSource::Spotify {
            self.send_spotify(PlayerCommand::Shuffle(shuffle)).await?;
        }
        Ok(self.current_info())
    }

    fn play_local_ref(&self, reference: &str) -> Result<()> {
        if self.active() == ActiveSource::Spotify {
            if let Ok(commands) = self.spotify_commands() {
                tokio::spawn(async move {
                    let _ = commands.send(PlayerCommand::Pause).await;
                });
            }
        }
        self.set_active(ActiveSource::Local);
        let queue = self.local_library.resolve_playback_queue(reference)?;
        self.local_player.play_queue(
            queue.entries,
            queue.start_index,
            self.local_library.clone(),
        )?;
        Ok(())
    }

    async fn play_spotify_ref(&self, reference: &str) -> Result<()> {
        self.local_player.stop();
        self.set_active(ActiveSource::Spotify);
        self.send_spotify(PlayerCommand::PlayRef(reference.to_string()))
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
    }

    fn spawn_status_forwarders(&self) {
        let mut local_status = self.local_player.player_info_channel();
        let local_active = self.active_source.clone();
        let local_sender = self.info_sender.clone();
        let local_controller = self.clone();
        tokio::spawn(async move {
            while local_status.changed().await.is_ok() {
                if *local_active.lock().unwrap() == ActiveSource::Local {
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
        let has_next = {
            let mut system_queue = self.system_queue.lock().unwrap();
            if system_queue.items.is_empty()
                || system_queue.current_index + 1 >= system_queue.items.len()
            {
                false
            } else {
                system_queue.current_index += 1;
                true
            }
        };

        if has_next {
            self.play_current_system_item().await?;
        }

        Ok(has_next)
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
        self.play_ref_without_system(&item.reference).await
    }
}
