use crate::spotify_sink::{ChannelSink, SinkEvent};
use gstreamer::event::SinkMessage;
use librespot_core::{Session, SpotifyId};
use librespot_metadata::artist::ArtistRole;
use librespot_metadata::audio::UniqueFields;
use librespot_metadata::{Metadata, Playlist, Track};
use librespot_playback::audio_backend::Sink;
use librespot_playback::config::PlayerConfig;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::mixer::{NoOpVolume, VolumeGetter};
use librespot_playback::player::{Player, PlayerEvent};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::option::Option;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::{channel, Receiver, Sender, UnboundedReceiver};
use tokio::sync::watch;
use tokio::time::{sleep, Instant};

use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub enum PlayerCommand {
    Playlist(String),
    Sleep(u32),
    Play,
    Pause,
    Next,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpotifyPlayerInfo {
    status: SpotifyPlayerState,
    shuffle: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<MusicMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sleep_timer: Option<u32>,
}

pub struct SpotifyPlayer {
    command_receiver: Receiver<PlayerCommand>,
    command_sender: Sender<PlayerCommand>,
    player_info_receiver: watch::Receiver<SpotifyPlayerInfo>,
    player_info_sender: watch::Sender<SpotifyPlayerInfo>,
    session: Session,
    state: SpotifyPlayerState,
    queue: VecDeque<SpotifyId>,
    player: Arc<Player>,
    shuffle: bool,
    current_song: Option<MusicMetadata>,
    volume: Arc<PlaybackVolume>,
    sleep_timer: Arc<SleepTimer>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
enum SpotifyPlayerState {
    Stopped,
    Playing,
    Paused,
}

pub struct PlaybackVolume {
    volume: Mutex<f64>,
}

impl PlaybackVolume {
    pub fn new(initial_volume: f64) -> Self {
        Self {
            volume: Mutex::new(initial_volume.min(1.0)),
        }
    }

    /// Set the volume (0-1)
    pub fn set_volume(&self, new_volume: f64) {
        let mut vol = self.volume.lock().unwrap();
        *vol = new_volume.min(1.0);
    }

    pub fn get_volume(&self) -> f64 {
        let vol = self.volume.lock().unwrap();
        *vol
    }
}

impl VolumeGetter for PlaybackVolume {
    fn attenuation_factor(&self) -> f64 {
        self.get_volume()
    }
}

pub struct ArcVolumeWrapper(Arc<PlaybackVolume>);

impl ArcVolumeWrapper {
    pub fn new(volume: Arc<PlaybackVolume>) -> Self {
        Self(volume)
    }
}

impl VolumeGetter for ArcVolumeWrapper {
    fn attenuation_factor(&self) -> f64 {
        self.0.attenuation_factor()
    }
}

impl SpotifyPlayer {
    pub fn new(session: Session, audio_sender: SyncSender<SinkEvent>) -> Self {
        let (sender, receiver) = channel::<PlayerCommand>(3);
        let sink = || Box::new(ChannelSink::new(audio_sender)) as Box<dyn Sink>;
        let player_config = PlayerConfig {
            bitrate: Default::default(),
            gapless: true,
            passthrough: true,
            normalisation: false,
            normalisation_type: Default::default(),
            normalisation_method: Default::default(),
            normalisation_pregain_db: 0.0,
            normalisation_threshold_dbfs: 0.0,
            normalisation_attack_cf: 0.0,
            normalisation_release_cf: 0.0,
            normalisation_knee_db: 0.0,
            ditherer: None,
        };
        // let volume_getter = Box::new(NoOpVolume);

        let volume = Arc::new(PlaybackVolume::new(0.5));
        let volume_clone = volume.clone();
        let volume_getter =
            Box::new(ArcVolumeWrapper::new(volume.clone())) as Box<dyn VolumeGetter + Send>;

        let player = Player::new(player_config, session.clone(), volume_getter, sink);
        let info = SpotifyPlayerInfo {
            status: SpotifyPlayerState::Stopped,
            metadata: None,
            shuffle: false,
            sleep_timer: None,
        };

        let (player_info_sender, player_info_receiver) = watch::channel(info);

        SpotifyPlayer {
            command_receiver: receiver,
            command_sender: sender,
            player_info_receiver,
            player_info_sender,
            state: SpotifyPlayerState::Stopped,
            queue: VecDeque::new(),
            session,
            player,
            shuffle: false,
            current_song: None,
            volume,
            sleep_timer: Arc::new(SleepTimer::new(volume_clone)),
        }
    }

    async fn set_sleep_timer(&mut self, delay: Duration) {
        let volume = self.volume.clone();
        let player = self.player.clone();

        self.sleep_timer
            .set_timer(delay, move || async move {
                fade_out_volume(volume, player).await;
            })
            .await;
    }

    async fn emit_player_state(&self) {
        let remaining = self.sleep_timer.remaining_time().await;
        let sleep_timer_secs = remaining.map(|d| d.as_secs() as u32);

        let state = SpotifyPlayerInfo {
            status: self.state.clone(),
            metadata: self.current_song.clone(),
            shuffle: self.shuffle,
            sleep_timer: sleep_timer_secs,
        };

        self.player_info_sender.send(state).unwrap();
    }

    pub fn command_channel(&self) -> Sender<PlayerCommand> {
        self.command_sender.clone()
    }

    pub fn player_info_channel(&self) -> watch::Receiver<SpotifyPlayerInfo> {
        self.player_info_receiver.clone()
    }

    pub async fn start(mut self) {
        log::info!("Starting player");

        let mut spotify_player_events: UnboundedReceiver<PlayerEvent> =
            self.player.get_player_event_channel();

        loop {
            // Wait for either a player command or an event from librespot
            tokio::select! {
                // Player commands
                Some(command) = self.command_receiver.recv() => {
                    log::info!("Received command: {:?}", command);
                    match command {
                        PlayerCommand::Playlist(p) => {
                            self.load_playlist_to_queue(&p).await;
                            self.play_next_song().await;
                        }
                        PlayerCommand::Pause => {
                            if matches!(self.state, SpotifyPlayerState::Playing) {
                                log::info!("Pausing");
                                self.player.pause();
                            }
                        }
                        PlayerCommand::Next => {
                            self.play_next_song().await;
                        }
                        PlayerCommand::Play => {
                            if let SpotifyPlayerState::Paused = self.state {
                                self.player.play();
                            }
                        }
                        PlayerCommand::Sleep(duration_s) => {
                            self.set_sleep_timer(Duration::from_secs(duration_s as u64)).await;
                            self.emit_player_state().await;
                        }
                        _ => {}
                    }
                }

                // Librespot events
                Some(event) = spotify_player_events.recv() => {
                    log::trace!("Received player event: {:?}", event);
                    match event {
                        PlayerEvent::Playing{ position_ms, .. } => self.set_state(SpotifyPlayerState::Playing).await,
                        PlayerEvent::Paused { position_ms, track_id, .. } => self.set_state(SpotifyPlayerState::Paused).await,
                        PlayerEvent::Stopped { .. } => self.set_state(SpotifyPlayerState::Stopped).await,
                        PlayerEvent::EndOfTrack { .. } => self.play_next_song().await,
                        PlayerEvent::TrackChanged { audio_item} => {
                            // log::trace!("Track changed to {:?}", audio_item);

                            let artist = match &audio_item.unique_fields {
                                UniqueFields::Track { artists, .. } => {
                                    artists.0.iter()
                                        .find(|a| a.role == ArtistRole::ARTIST_ROLE_MAIN_ARTIST)
                                        .or_else(|| artists.0.first())
                                        .map(|a| a.name.clone())
                                        .unwrap_or_else(|| "Unknown Artist".to_string())
                                },
                                _ => "Unknown Artist".to_string(),
                            };

                            let metadata = MusicMetadata {
                                artist,
                                title: audio_item.name.clone(),
                                artwork_url: audio_item.covers.get(0)
                                    .map(|c| c.url.clone())
                                    .unwrap_or_else(|| "".to_string()),
                            };
                            self.current_song = Some(metadata);
                            self.emit_player_state().await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    async fn play_next_song(&mut self) {
        if let Some(next_track_id) = self.queue.pop_front() {
            if let Ok(_) = Track::get(&self.session, &next_track_id).await {
                self.player.load(next_track_id, true, 0);
            }
        } else {
            self.set_state(SpotifyPlayerState::Stopped).await;
        }
    }

    async fn set_state(&mut self, state: SpotifyPlayerState) {
        if state != self.state {
            self.state = state;
            self.emit_player_state().await;
        }
    }

    async fn load_playlist_to_queue(&mut self, playlist_id: &str) {
        let plist_uri = SpotifyId::from_uri(&format!("{}", playlist_id))
            .expect("Spotify URI could not be parsed.");

        let play_list = Playlist::get(&self.session, &plist_uri).await.unwrap();
        log::trace!("Playlist Uri {}", play_list.name());
        let mut tracks = play_list.tracks();
        self.queue.clear();
        self.queue.extend(tracks.map(|t| *t));
    }
}

struct SleepTimerInner {
    handle: Option<JoinHandle<()>>,
    initial_volume: f64,
    deadline: Option<Instant>,
}

pub struct SleepTimer {
    inner: TokioMutex<SleepTimerInner>,
    volume: Arc<PlaybackVolume>,
}

impl SleepTimer {
    pub fn new(volume: Arc<PlaybackVolume>) -> Self {
        Self {
            inner: TokioMutex::new(SleepTimerInner {
                handle: None,
                initial_volume: volume.get_volume(),
                deadline: None,
            }),
            volume,
        }
    }

    /// Sets a new sleep timer. When the timer expires (after `delay`), the provided `fade_out_fn`
    /// will be run.
    pub async fn set_timer<F, Fut>(&self, delay: Duration, fade_out_fn: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut inner = self.inner.lock().await;
        if let Some(handle) = inner.handle.take() {
            handle.abort();
            self.volume.set_volume(inner.initial_volume);
        }

        if delay.is_zero() {
            inner.deadline = None;
            return;
        }

        inner.initial_volume = self.volume.get_volume();
        inner.deadline = Some(Instant::now() + delay);

        let handle = tokio::spawn(async move {
            sleep(delay).await;
            fade_out_fn().await;
        });
        inner.handle = Some(handle);
    }

    /// Returns the remaining time until the timer expires, if a timer is active.
    pub async fn remaining_time(&self) -> Option<Duration> {
        let inner = self.inner.lock().await;
        if let Some(deadline) = inner.deadline {
            let now = Instant::now();
            if deadline > now {
                return Some(deadline - now);
            }
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MusicMetadata {
    artist: String,
    title: String,
    artwork_url: String,
}

async fn fade_out_volume(volume: Arc<PlaybackVolume>, player: Arc<Player>) {
    log::info!("Fading out volume");
    let fade_duration = Duration::from_secs(10);
    let steps = 100;
    let step_duration = fade_duration / steps;
    let initial_volume = volume.get_volume();

    for step in 0..steps {
        log::info!("Fading step {}", step);
        let fraction = (step + 1) as f64 / steps as f64;
        let new_volume = initial_volume * (1.0 - fraction);
        volume.set_volume(new_volume);
        sleep(step_duration).await;
    }
    log::info!("Fading done");
    player.pause();

    // Restore volume after pausing
    sleep(Duration::from_secs(1)).await;
    volume.set_volume(initial_volume);
}
