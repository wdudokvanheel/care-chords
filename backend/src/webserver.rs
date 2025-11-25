use crate::spotify_client::SpotifyClient;
use crate::spotify_player::{PlayerCommand, SpotifyPlayerInfo};
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::watch;
use warp::http::{Response, StatusCode};
use warp::hyper::Body;
use warp::{Filter, Rejection, Reply};

#[derive(serde::Deserialize)]
struct PlaylistRequest {
    uri: String,
}

#[derive(serde::Deserialize)]
struct SleepTimerRequest {
    timer: u32,
}

#[derive(serde::Deserialize)]
struct ShuffleRequest {
    shuffle: bool,
}

pub fn start_http_server(spotify: Arc<SpotifyClient>) {
    log::info!("Starting server @ :7755");
    let routes = create_routes(spotify);
    tokio::spawn(async move {
        warp::serve(routes).run(([0, 0, 0, 0], 7755)).await;
    });
}
fn create_routes(
    spotify: Arc<SpotifyClient>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let command_channel = spotify.player_command_channel();
    let info_channel = spotify.player_info_channel();

    let command_filter = warp::any().map(move || command_channel.clone());
    let info_filter = warp::any().map(move || info_channel.clone());

    let playlist_route = warp::path("playlist")
        .and(warp::post())
        .and(warp::body::json::<PlaylistRequest>())
        .and(command_filter.clone())
        .and_then(handle_playlist);

    let play_route = warp::path("play")
        .and(warp::post())
        .and(command_filter.clone())
        .and_then(handle_play);

    let pause_route = warp::path("pause")
        .and(warp::post())
        .and(command_filter.clone())
        .and_then(handle_pause);

    let next_route = warp::path("next")
        .and(warp::post())
        .and(command_filter.clone())
        .and_then(handle_next);

    let sleep_route = warp::path("sleep")
        .and(warp::post())
        .and(warp::body::json::<SleepTimerRequest>())
        .and(command_filter.clone())
        .and_then(handle_sleep);

    let shuffle_route = warp::path("shuffle")
        .and(warp::post())
        .and(warp::body::json::<ShuffleRequest>())
        .and(command_filter.clone())
        .and(info_filter.clone())
        .and_then(handle_shuffle);

    let status_route = warp::path("status")
        .and(warp::get())
        .and(info_filter.clone())
        .and_then(handle_status);

    // New SSE route that streams the player state:
    let status_stream_route = warp::path("status_stream")
        .and(warp::get())
        .and(info_filter)
        .map(|mut info_channel: watch::Receiver<SpotifyPlayerInfo>| {
            // Create an async stream that yields an SSE event on every state change.
            let event_stream: futures_util::stream::BoxStream<
                'static,
                Result<warp::sse::Event, std::convert::Infallible>,
            > = async_stream::stream! {
                // Immediately yield the current state on connection.
                let initial_state = info_channel.borrow().clone();
                let mut last_emitted = serde_json::to_string(&initial_state)
                    .unwrap_or_else(|_| "{}".to_string());
                yield Ok(warp::sse::Event::default().data(last_emitted.clone()));

                // Then, yield subsequent updates only if they differ from the last emitted state.
                loop {
                    if info_channel.changed().await.is_err() {
                        break;
                    }
                    let current_state = info_channel.borrow().clone();
                    let current_json = serde_json::to_string(&current_state)
                        .unwrap_or_else(|_| "{}".to_string());
                    // Only yield if the new JSON differs from what was last sent.
                    if current_json != last_emitted {
                        last_emitted = current_json.clone();
                        yield Ok(warp::sse::Event::default().data(current_json));
                    }
                }
            }
            .boxed();

            // Return the SSE reply with a keep-alive stream.
            warp::sse::reply(warp::sse::keep_alive().stream(event_stream))
        });

    playlist_route
        .or(play_route)
        .or(pause_route)
        .or(next_route)
        .or(status_route)
        .or(sleep_route)
        .or(shuffle_route)
        .or(status_stream_route)
        .boxed()
}

async fn handle_status(
    info_channel: watch::Receiver<SpotifyPlayerInfo>,
) -> Result<impl Reply, Rejection> {
    let info = info_channel.borrow().clone(); // Get the latest player info

    Ok(warp::reply::json(&info))
}

async fn handle_playlist(
    req: PlaylistRequest,
    client: Sender<PlayerCommand>,
) -> Result<Response<Body>, Rejection> {
    client
        .send(PlayerCommand::Playlist(req.uri))
        .await
        .expect("Failed to send player command");

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "ok" })),
        StatusCode::OK,
    )
    .into_response())
}

async fn handle_sleep(
    req: SleepTimerRequest,
    client: Sender<PlayerCommand>,
) -> Result<Response<Body>, Rejection> {
    client
        .send(PlayerCommand::Sleep(req.timer))
        .await
        .expect("Failed to send player command");

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "ok" })),
        StatusCode::OK,
    )
    .into_response())
}

async fn handle_shuffle(
    req: ShuffleRequest,
    client: Sender<PlayerCommand>,
    info_channel: watch::Receiver<SpotifyPlayerInfo>,
) -> Result<Response<Body>, Rejection> {
    client
        .send(PlayerCommand::Shuffle(req.shuffle))
        .await
        .expect("Failed to send shuffle command");

    Ok(warp::reply::with_status(
        warp::reply::json(&info_channel.borrow().clone()),
        StatusCode::OK,
    )
    .into_response())
}

async fn handle_play(client: Sender<PlayerCommand>) -> Result<Response<Body>, Rejection> {
    client
        .send(PlayerCommand::Play)
        .await
        .expect("Failed to send play command");

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "playing" })),
        StatusCode::OK,
    )
    .into_response())
}

async fn handle_pause(client: Sender<PlayerCommand>) -> Result<Response<Body>, Rejection> {
    client
        .send(PlayerCommand::Pause)
        .await
        .expect("Failed to send pause command");

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "paused" })),
        StatusCode::OK,
    )
    .into_response())
}

async fn handle_next(client: Sender<PlayerCommand>) -> Result<Response<Body>, Rejection> {
    client
        .send(PlayerCommand::Next)
        .await
        .expect("Failed to send next command");

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "success" })),
        StatusCode::OK,
    )
    .into_response())
}
