use std::time::Duration;

pub fn send_spotify_message(message: &str) {
    let spotify_dest = get_dbus_spotify_connection();
    if spotify_dest.is_none() {
        return;
    }
    let (conn, spotify_dest) = spotify_dest.unwrap();

    // Send the Pause command to Spotify
    let pause_msg = Message::new_method_call(
        spotify_dest,
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
        message,
    )
        .expect("Failed to call dbus method");

    let response = conn.send_with_reply_and_block(pause_msg, Duration::from_millis(5000))
        .expect("Failed to send message to dbus");
    println!("Pause command sent, response: {:?}", response);
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
