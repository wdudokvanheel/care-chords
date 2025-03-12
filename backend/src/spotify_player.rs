use crate::spotify_old::MusicMetadata;
use crate::spotify_sink::{ChannelSink, SinkEvent};
use gstreamer::event::SinkMessage;
use librespot_core::{Session, SpotifyId};
use librespot_metadata::{Metadata, Playlist, Track};
use librespot_playback::audio_backend::Sink;
use librespot_playback::config::PlayerConfig;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::mixer::NoOpVolume;
use librespot_playback::player::{Player, PlayerEvent};
use std::collections::VecDeque;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Receiver, Sender, UnboundedReceiver};
use tokio::sync::watch;
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub enum PlayerCommand {
    Playlist(String),
    Play,
    Pause,
    Next,
}

#[derive(Clone, Debug)]
pub struct SpotifyPlayerInfo {
    status: SpotifyPlayerState,
    music_metadata: Option<MusicMetadata>,
    shuffle: bool,
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
}

#[derive(Clone, Debug)]
enum SpotifyPlayerState {
    Stopped,
    Playing,
    Paused(SpotifyId, u32),
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
        let volume_getter = Box::new(NoOpVolume);

        let player = Player::new(player_config, session.clone(), volume_getter, sink);
        let info = SpotifyPlayerInfo {
            status: SpotifyPlayerState::Stopped,
            music_metadata: None,
            shuffle: false,
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
        }
    }

    fn emit_player_state(&self){
        let state = SpotifyPlayerInfo {
            status: self.state.clone(),
            music_metadata: self.current_song.clone(),
            shuffle: self.shuffle,
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
                            if let SpotifyPlayerState::Paused(id, position_ms) = self.state {
                                log::info!("Resuming playback @ {}", position_ms);
                                self.player.play();
                            }
                        }
                        _ => {}
                    }
                }

                // Librespot events
                Some(event) = spotify_player_events.recv() => {
                    log::trace!("Received player event: {:?}", event);
                    match event {
                        PlayerEvent::Playing{ position_ms, .. } => self.set_state(SpotifyPlayerState::Playing).await,
                        PlayerEvent::Paused { position_ms, track_id, .. } => self.set_state(SpotifyPlayerState::Paused(track_id, position_ms)).await,
                        PlayerEvent::Stopped { .. } => self.set_state(SpotifyPlayerState::Stopped).await,
                        PlayerEvent::EndOfTrack { .. } => self.play_next_song().await,
                        _ => {}
                    }
                }
            }
        }
    }

    async fn play_next_song(&mut self) {
        if let Some(next_track_id) = self.queue.pop_front() {
            if let Ok(track) = Track::get(&self.session, &next_track_id).await {
                self.player.load(next_track_id, true, 0);
            }
        } else {
            self.set_state(SpotifyPlayerState::Stopped).await;
        }
    }

    async fn set_state(&mut self, state: SpotifyPlayerState) {
        self.state = state;
        self.emit_player_state();
    }

    async fn load_playlist_to_queue(&mut self, playlist_id: &str) {
        let plist_uri = SpotifyId::from_uri(&format!("spotify:playlist:{}", playlist_id))
            .expect("Spotify URI could not be parsed.");

        let play_list = Playlist::get(&self.session, &plist_uri).await.unwrap();
        log::trace!("Playlist Uri {}", play_list.name());
        let mut tracks = play_list.tracks();
        self.queue.clear();
        self.queue.extend(tracks.map(|t| *t));
    }
}
