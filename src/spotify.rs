use dbus::arg::{cast, AppendAll, PropMap, ReadAll, RefArg, Variant};
use dbus::nonblock::stdintf::org_freedesktop_dbus::Properties;
use dbus::nonblock::{Proxy, SyncConnection};
use dbus::strings::Interface;
use dbus::strings::Member;
use dbus::Error;
use dbus_tokio::connection;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

const DBUS_TIMEOUT: Duration = Duration::from_secs(2);
const METHOD_CALL_MAX_RETRIES: usize = 3;

pub struct SpotifyDBusClient {
    dbus_connection: Arc<SyncConnection>,
    spotify_destination: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicMetadata {
    artist: String,
    title: String,
    artwork_url: String,
}

impl SpotifyDBusClient {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (resource, connection) = connection::new_session_sync()?;

        tokio::spawn(async {
            let err = resource.await;
            log::error!("Lost connection to D-Bus: {}", err);
            panic!("Lost connection to D-Bus: {}", err);
        });

        let spotify_destination = Self::find_spotify_destination(connection.clone()).await?;

        Ok(Self {
            dbus_connection: connection,
            spotify_destination,
        })
    }

    async fn find_spotify_destination(
        connection: Arc<SyncConnection>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let proxy = Proxy::new(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            DBUS_TIMEOUT,
            connection,
        );

        let (names,): (Vec<String>,) = proxy
            .method_call("org.freedesktop.DBus", "ListNames", ())
            .await
            .expect("Failed to call DBus list");

        names
            .iter()
            .find(|name| name.contains("org.mpris.MediaPlayer2.spotify"))
            .map(|s| s.to_string())
            .ok_or_else(|| Box::<dyn std::error::Error>::from("Spotify destination not found"))
    }

    pub async fn send_player_message(&mut self, message: &str) {
        debug!("Sending spotify message: {}", message);

        let _: Result<(), _> = self
            .spotify_method_with_retry(
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                message,
                (),
            )
            .await;
    }

    pub async fn transfer_audio_playback(&mut self) {
        debug!("Transferring Spotify playback to local device");

        let _: Result<(), _> = self
            .spotify_method_with_retry(
                "/rs/spotifyd/Controls",
                "rs.spotifyd.Controls",
                "TransferPlayback",
                (),
            )
            .await;
    }

    pub async fn is_playing(&mut self) -> bool {
        let result: Result<(Variant<String>,), _> = self
            .spotify_method_with_retry(
                "/org/mpris/MediaPlayer2",
                "org.freedesktop.DBus.Properties",
                "Get",
                (
                    "org.mpris.MediaPlayer2.Player".to_string(),
                    "PlaybackStatus".to_string(),
                ),
            )
            .await;

        match result {
            Ok((variant,)) => variant.0 == "Playing",
            Err(err) => {
                error!("Error checking playback status: {:?}", err);
                false
            }
        }
    }

    pub async fn is_selected_playback(&mut self) -> bool {
        let result: Result<(Variant<String>,), _> = self
            .spotify_method_with_retry(
                "/org/mpris/MediaPlayer2",
                "org.freedesktop.DBus.Properties",
                "Get",
                (
                    "org.mpris.MediaPlayer2.Player".to_string(),
                    "PlaybackStatus".to_string(),
                ),
            )
            .await;

        match result {
            Ok((variant,)) => variant.0 == "Playing" || variant.0 == "Paused",
            Err(err) => {
                error!("Error checking playback status: {:?}", err);
                false
            }
        }
    }

    pub async fn get_current_song_metadata(&mut self) -> Option<MusicMetadata> {
        let proxy = Proxy::new(
            self.spotify_destination.clone(),
            "/org/mpris/MediaPlayer2",
            DBUS_TIMEOUT,
            self.dbus_connection.clone(),
        );

        if let Some(properties) = proxy.get_all("org.mpris.MediaPlayer2.Player").await.ok() {
            if let Some(Variant(metadata_variant)) = properties.get("Metadata") {
                if let Some(metadata_map) = cast::<PropMap>(&*metadata_variant) {
                    let title = metadata_map
                        .get("xesam:title")
                        .and_then(|v| v.0.as_str())
                        .unwrap_or("")
                        .to_string();

                    let artwork_url = metadata_map
                        .get("mpris:artUrl")
                        .and_then(|v| v.0.as_str())
                        .unwrap_or("")
                        .to_string();

                    let artist = metadata_map
                        .get("xesam:artist")
                        .and_then(|a| a.0.as_iter())
                        .map(|iter| {
                            iter.filter_map(|ref_arg| ref_arg.as_str().map(|s| s.to_string()))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_else(Vec::new)
                        .join(" & ");

                    return Some(MusicMetadata {
                        artist,
                        title,
                        artwork_url,
                    });
                }
            } else {
                error!("Failed to cast metadata_variant to PropMap");
            }
            error!("Failed to get metadata")
        }

        None
    }

    pub async fn play_uri(&mut self, uri: &str) {
        debug!("Requesting playback of {}", uri);

        let playlist_uri = if !uri.starts_with("spotify:") {
            format!("spotify:{}", uri)
        } else {
            uri.to_string()
        };

        let _: Result<(), _> = self
            .spotify_method_with_retry(
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                "OpenUri",
                (playlist_uri,),
            )
            .await;
    }

    async fn spotify_method_with_retry<'i, 'm, R, A, I, M>(
        &mut self,
        path: &str,
        interface: I,
        method: M,
        args: A,
    ) -> Result<R, Error>
    where
        R: 'static + ReadAll,
        A: AppendAll + Clone,
        I: Into<Interface<'i>> + Clone,
        M: Into<Member<'m>> + Clone,
    {
        let interface: Interface<'i> = interface.into();
        let method: Member<'m> = method.into();
        let mut retries = 0;

        loop {
            let args_clone = args.clone();
            let proxy = Proxy::new(
                self.spotify_destination.clone(),
                path,
                DBUS_TIMEOUT,
                self.dbus_connection.clone(),
            );
            let result = proxy.method_call(&interface, &method, args_clone).await;

            match result {
                Ok(res) => return Ok(res),
                Err(err) => {
                    warn!("Error calling method: {:?} retrying in 1 second", err);
                    if retries < METHOD_CALL_MAX_RETRIES {
                        retries += 1;

                        // Update spotify destination in case its PID changed
                        match Self::find_spotify_destination(self.dbus_connection.clone()).await {
                            Ok(new_dest) => {
                                self.spotify_destination = new_dest;
                            }
                            Err(update_err) => {
                                error!("Failed to update Spotify destination: {:?}", update_err);
                            }
                        }

                        tokio::time::sleep(Duration::from_millis(1000)).await;
                        debug!("Retrying method call (attempt {})", retries + 1);
                    } else {
                        return Err(err);
                    }
                }
            }
        }
    }
}
