use crate::spotify::{MusicMetadata, SpotifyDBusClient};
use crate::webserver;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch::Sender;
use tokio::sync::{watch, Mutex};
use warp::http::StatusCode;
use warp::Filter;

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerStateDto {
    playing: bool,
    metadata: Option<MusicMetadata>,
    sleep_timer: Option<u64>,
}

fn create_routes(
    sleep_timer_tx: Sender<Option<Instant>>,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sleep_route = warp::path("sleep")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_sleep_sender(sleep_timer_tx.clone()))
        .and(with_sleep_time(sleep_start_time.clone()))
        .and(with_spotify_client(spotify_client.clone()))
        .and_then(handle_sleep_request);

    let control_route = warp::path("control")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_sleep_time(sleep_start_time.clone()))
        .and(with_spotify_client(spotify_client.clone()))
        .and_then(handle_control_request);

    let playback_route = warp::path("play")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_spotify_client(spotify_client.clone()))
        .and_then(handle_playback_request);

    // Update the state route to pass the sleep_start_time
    let state_route = warp::path("status")
        .and(warp::get())
        .and(with_sleep_time(sleep_start_time.clone()))
        .and(with_spotify_client(spotify_client.clone()))
        .and_then(handle_state_request);

    sleep_route
        .or(control_route)
        .or(playback_route)
        .or(state_route)
        .boxed()
}

pub fn start_server(
    sleep_timer_tx: Sender<Option<Instant>>,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) {
    tokio::spawn(async move {
        println!("Starting server @ :7755");
        let routes = create_routes(sleep_timer_tx, sleep_start_time, spotify_client);
        warp::serve(routes).run(([0, 0, 0, 0], 7755)).await;
    });
}

pub async fn create_playerstate_dto(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> PlayerStateDto {
    let (playing, music) = {
        let mut spotify = spotify_client.lock().await;
        (spotify.is_playing().await, spotify.get_current_song_metadata().await)
    };

    // Calculate remaining sleep time if the timer is active
    let sleep_time_left = {
        let lock = sleep_start_time.lock().await;
        if let Some(end_time) = *lock {
            let now = Instant::now();
            if now < end_time {
                Some((end_time - now).as_secs())
            } else {
                None
            }
        } else {
            None
        }
    };

    PlayerStateDto {
        playing,
        metadata: music,
        sleep_timer: sleep_time_left,
    }
}

async fn handle_playback_request(
    body: Value,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(uri) = body.get("uri").and_then(|t| t.as_str()) {
        {
            let mut spotify = spotify_client.lock().await;
            if !spotify.is_selected_playback().await {
                spotify.transfer_audio_playback().await;
                tokio::time::sleep(Duration::from_millis(1500)).await;
            }
            spotify.play_uri(uri).await;
        }

        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "status": "ok" })),
            StatusCode::OK,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
        StatusCode::BAD_REQUEST,
    ))
}

async fn handle_control_request(
    body: Value,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(state) = body.get("action").and_then(|t| t.as_str()) {
        {
            let mut spotify = spotify_client.lock().await;
            match state.to_lowercase().as_str() {
                "play" => {
                    if !spotify.is_selected_playback().await {
                        spotify.transfer_audio_playback().await;
                        tokio::time::sleep(Duration::from_millis(1500)).await;
                    }
                    spotify.send_player_message("Play").await;
                }
                "pause" => spotify.send_player_message("Pause").await,
                "next" => spotify.send_player_message("Next").await,
                "previous" => spotify.send_player_message("Previous").await,
                _ => {}
            }
        }

        let state = webserver::create_playerstate_dto(sleep_start_time, spotify_client).await;

        return Ok(warp::reply::with_status(
            warp::reply::json(&state),
            StatusCode::OK,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
        StatusCode::BAD_REQUEST,
    ))
}

async fn handle_state_request(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = webserver::create_playerstate_dto(sleep_start_time, spotify_client).await;

    Ok(warp::reply::with_status(
        warp::reply::json(&state),
        StatusCode::OK,
    ))
}

async fn handle_sleep_request(
    body: Value,
    sleep_tx: watch::Sender<Option<Instant>>,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(sleep_timer) = body.get("timer").and_then(|t| t.as_u64()) {
        {
            let mut spotify = spotify_client.lock().await;
            if !spotify.is_playing().await {
                spotify.send_player_message("Play").await;
            }
        }

        let end_time = Instant::now() + Duration::from_secs(sleep_timer);

        // Update sleep_start_time to the new end time
        let mut start_time_lock = sleep_start_time.lock().await;
        *start_time_lock = Some(end_time);

        // Send the new end time to the channel, canceling the old timer
        let _ = sleep_tx.send(Some(end_time));

        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "status": "timer started" })),
            StatusCode::OK,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": "invalid request" })),
        StatusCode::BAD_REQUEST,
    ))
}

fn with_sleep_sender(
    sender: watch::Sender<Option<Instant>>,
) -> impl Filter<Extract = (watch::Sender<Option<Instant>>,), Error = std::convert::Infallible> + Clone
{
    warp::any().map(move || sender.clone())
}

fn with_sleep_time(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
) -> impl Filter<Extract = (Arc<Mutex<Option<Instant>>>,), Error = std::convert::Infallible> + Clone
{
    warp::any().map(move || sleep_start_time.clone())
}

fn with_spotify_client(
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> impl Filter<Extract = (Arc<Mutex<SpotifyDBusClient>>,), Error = std::convert::Infallible> + Clone
{
    warp::any().map(move || spotify_client.clone())
}
