use dbus::arg::messageitem::MessageItem;
use dbus::arg::Variant;
use dbus::blocking::BlockingSender;
use dbus::blocking::Connection;
use dbus::Message;
use std::time::Duration;

pub fn send_spotify_message(message: &str) {
    let spotify_dest = get_dbus_spotify_connection();
    if spotify_dest.is_none() {
        return;
    }
    let (conn, spotify_dest) = spotify_dest.unwrap();

    println!("Sending spotify message: {}", message);

    let pause_msg = Message::new_method_call(
        spotify_dest,
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
        message,
    )
        .expect("Failed to call dbus method");

    let _ = conn.send_with_reply_and_block(pause_msg, Duration::from_millis(5000))
        .expect("Failed to send message to dbus");
}

pub fn transfer_playback() {
    let spotify_dest = get_dbus_spotify_connection();
    if spotify_dest.is_none() {
        return;
    }
    let (conn, spotify_dest) = spotify_dest.unwrap();

    println!("Transferring Spotify playback to local device");

    let pause_msg = Message::new_method_call(
        spotify_dest,
        "/rs/spotifyd/Controls",
        "rs.spotifyd.Controls",
        "TransferPlayback",
    )
        .expect("Failed to call dbus method");

    let _ = conn.send_with_reply_and_block(pause_msg, Duration::from_millis(5000))
        .expect("Failed to send message to dbus");
}


pub fn playback(uri: &str) {
    println!("Requesting playback of {}", uri);

    let spotify_dest = get_dbus_spotify_connection();
    if spotify_dest.is_none() {
        return;
    }
    let (conn, spotify_dest) = spotify_dest.unwrap();

    // Spotify URI for the playlist (replace with your playlist URI)
    let playlist_uri =
        if !uri.starts_with("spotify:") {
            format!("spotify:{}", uri)
        } else {
            uri.to_string()
        };

    let mut message = Message::new_method_call(
        spotify_dest,
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
        "OpenUri",
    )
        .expect("Failed to call dbus method");

    message.append_items(&[
        MessageItem::Str(playlist_uri.to_string())]
    );

    let _ = conn.send_with_reply_and_block(message, Duration::from_millis(5000))
        .expect("Failed to send message to dbus");
}

pub fn is_spotify_playing() -> bool {
    let spotify_dest = get_dbus_spotify_connection();
    if spotify_dest.is_none() {
        return false;
    }
    let (conn, spotify_dest) = spotify_dest.unwrap();

    let playback_status_msg = Message::new_method_call(
        spotify_dest,
        "/org/mpris/MediaPlayer2",
        "org.freedesktop.DBus.Properties",
        "Get",
    )
        .expect("Failed to create method call")
        .append2("org.mpris.MediaPlayer2.Player", "PlaybackStatus");

    let response = conn.send_with_reply_and_block(playback_status_msg, Duration::from_millis(5000))
        .expect("Failed to send message to dbus");

    if let Some(variant) = response.get1::<Variant<&str>>() {
        return variant.0 == "Playing";
    }

    return false;
}

fn get_dbus_spotify_connection() -> Option<(Connection, String)> {
    // Establish a connection to the D-Bus session
    let conn = Connection::new_session()
        .expect("Failed to create a dbus connection");

    // Call ListNames to get all service names on the session bus
    let proxy = conn.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", Duration::from_millis(5000));
    let (names, ): (Vec<String>,) = proxy.method_call("org.freedesktop.DBus", "ListNames", ())
        .expect("Failed to list dbus names");

    // Find the Spotify destination that contains the word 'spotify'
    let spotify_dest = names.iter().find(|name| name.contains("org.mpris.MediaPlayer2.spotify"));

    // Check if Spotify destination is found
    Some((conn, spotify_dest?.to_string()))
}
