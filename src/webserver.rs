use crate::spotify::{MusicMetadata, PlayerStatus, SpotifyDBusClient};
use crate::webserver;
use dbus::Error;
use log::error;
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
    shuffle: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<MusicMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

    let shuffle_route = warp::path("shuffle")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_sleep_time(sleep_start_time.clone()))
        .and(with_spotify_client(spotify_client.clone()))
        .and_then(handle_shuffle_request);

    sleep_route
        .or(control_route)
        .or(playback_route)
        .or(state_route)
        .or(shuffle_route)
        .boxed()
}

pub fn start_server(
    sleep_timer_tx: Sender<Option<Instant>>,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) {
    tokio::spawn(async move {
        log::info!("Starting server @ :7755");
        let routes = create_routes(sleep_timer_tx, sleep_start_time, spotify_client);
        warp::serve(routes).run(([0, 0, 0, 0], 7755)).await;
    });
}

pub async fn create_playerstate_dto(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> Result<PlayerStateDto, Error> {
    // Calculate remaining sleep time if the timer is active
    let sleep_time_left = {
        let lock = sleep_start_time.lock().await;
        lock.as_ref().and_then(|end_time| {
            let now = Instant::now();
            (now < *end_time).then_some((*end_time - now).as_secs())
        })
    };

    // Fetch player status
    let player = spotify_client.lock().await.status().await;

    match player {
        Err(err) => Err(Error::new_custom(
            "com.bitechular.PlayerState",
            &format!("Failed to get status: {}", err),
        )),
        Ok(PlayerStatus {
            playing,
            shuffle,
            metadata,
        }) => Ok(PlayerStateDto {
            sleep_timer: sleep_time_left,
            playing,
            shuffle,
            metadata,
        }),
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
    }
    handle_state_request(sleep_start_time, spotify_client).await
}

async fn handle_state_request(
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = webserver::create_playerstate_dto(sleep_start_time, spotify_client).await;

    match state {
        Ok(state) => Ok(warp::reply::with_status(
            warp::reply::json(&state),
            StatusCode::OK,
        )),
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "server error" })),
            StatusCode::SERVICE_UNAVAILABLE,
        )),
    }
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

async fn handle_shuffle_request(
    body: Value,
    sleep_start_time: Arc<Mutex<Option<Instant>>>,
    spotify_client: Arc<Mutex<SpotifyDBusClient>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(shuffle_state) = body.get("shuffle").and_then(|s| s.as_bool()) {
        {
            let mut spotify = spotify_client.lock().await;
            if !spotify.is_selected_playback().await {
                spotify.transfer_audio_playback().await;
                tokio::time::sleep(Duration::from_millis(1500)).await;
            }

            match spotify.set_shuffle(shuffle_state).await {
                Err(e) => {
                    error!("Failed to set shuffle state: {}", e);
                }
                Ok(_) => {}
            }
        }
    }

    handle_state_request(sleep_start_time.clone(), spotify_client.clone()).await
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
