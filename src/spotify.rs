use dbus::arg::messageitem::MessageItem;
use dbus::arg::{RefArg, Variant};
use dbus::blocking::BlockingSender;
use dbus::blocking::Connection;
use dbus::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicMetadata {
    artist: String,
    title: String,
    artwork_url: String,
}

pub struct SpotifyDBusClient {
    conn: Connection,
    spotify_dest: String,
}

impl SpotifyDBusClient {
    pub fn new() -> Option<Self> {
        let conn = Connection::new_session().expect("Failed to create a dbus connection");

        let proxy = conn.with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            Duration::from_millis(5000),
        );
        let (names,): (Vec<String>,) = proxy
            .method_call("org.freedesktop.DBus", "ListNames", ())
            .expect("Failed to list dbus names");

        let spotify_dest = names
            .iter()
            .find(|name| name.contains("org.mpris.MediaPlayer2.spotify"))
            .map(|s| s.to_string())?;

        Some(Self { conn, spotify_dest })
    }

    pub fn send_player_message(&self, message: &str) {
        println!("Sending spotify message: {}", message);

        let pause_msg = Message::new_method_call(
            &self.spotify_dest,
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
            message,
        )
            .expect("Failed to call dbus method");

        let _ = self
            .conn
            .send_with_reply_and_block(pause_msg, Duration::from_millis(5000))
            .expect("Failed to send message to dbus");
    }

    pub fn transfer_audio_playback(&self) {
        println!("Transferring Spotify playback to local device");

        let transfer_msg = Message::new_method_call(
            &self.spotify_dest,
            "/rs/spotifyd/Controls",
            "rs.spotifyd.Controls",
            "TransferPlayback",
        )
            .expect("Failed to call dbus method");

        let _ = self
            .conn
            .send_with_reply_and_block(transfer_msg, Duration::from_millis(5000))
            .expect("Failed to send message to dbus");
    }

    pub fn play_uri(&self, uri: &str) {
        println!("Requesting playback of {}", uri);

        let playlist_uri = if !uri.starts_with("spotify:") {
            format!("spotify:{}", uri)
        } else {
            uri.to_string()
        };

        let mut message = Message::new_method_call(
            &self.spotify_dest,
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
            "OpenUri",
        )
            .expect("Failed to call dbus method");

        message.append_items(&[MessageItem::Str(playlist_uri.to_string())]);

        let _ = self
            .conn
            .send_with_reply_and_block(message, Duration::from_millis(5000))
            .expect("Failed to send message to dbus");
    }

    pub fn is_playing(&self) -> bool {
        let playback_status_msg = Message::new_method_call(
            &self.spotify_dest,
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
            "Get",
        )
            .expect("Failed to create method call")
            .append2("org.mpris.MediaPlayer2.Player", "PlaybackStatus");

        let response = self
            .conn
            .send_with_reply_and_block(playback_status_msg, Duration::from_millis(5000))
            .expect("Failed to send message to dbus");

        if let Some(variant) = response.get1::<Variant<&str>>() {
            return variant.0 == "Playing";
        }

        false
    }

    pub fn is_selected_playback(&self) -> bool {
        let playback_status_msg = Message::new_method_call(
            &self.spotify_dest,
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
            "Get",
        )
            .expect("Failed to create method call")
            .append2("org.mpris.MediaPlayer2.Player", "PlaybackStatus");

        let response = self
            .conn
            .send_with_reply_and_block(playback_status_msg, Duration::from_millis(5000))
            .expect("Failed to send message to dbus");

        if let Some(variant) = response.get1::<Variant<&str>>() {
            return variant.0 == "Playing" || variant.0 == "Paused";
        }

        false
    }

    pub fn get_current_song_metadata(&self) -> Option<MusicMetadata> {
        let metadata_msg = Message::new_method_call(
            &self.spotify_dest,
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
            "Get",
        )
            .expect("Failed to create method call")
            .append2("org.mpris.MediaPlayer2.Player", "Metadata");

        let response = self
            .conn
            .send_with_reply_and_block(metadata_msg, Duration::from_millis(5000))
            .expect("Failed to send message to dbus");

        if let Some(Variant(dict)) = response.get1::<Variant<HashMap<String, Variant<Box<dyn RefArg>>>>>(){
            let mut artist = String::new();
            let mut title = String::new();
            let mut artwork_url = String::new();

            for (key, value) in dict {
                match key.as_str() {
                    "xesam:artist" => {
                        if let Some(artist_vec) = value.0.as_iter() {
                            if let Some(first_artist) = artist_vec.filter_map(|v| v.as_str()).next() {
                                artist = first_artist.to_string();
                            }
                        }
                    }
                    "xesam:title" => {
                        if let Some(title_str) = value.0.as_str() {
                            title = title_str.to_string();
                        }
                    }
                    "mpris:artUrl" => {
                        if let Some(url_str) = value.0.as_str() {
                            artwork_url = url_str.to_string();
                        }
                    }
                    _ => {}
                }
            }

            return Some(MusicMetadata {
                artist,
                title,
                artwork_url,
            });
        }

        None
    }
}
