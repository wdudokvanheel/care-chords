use crate::spotify_client::SpotifyClient;
use crate::spotify_player::{PlayerCommand, SpotifyPlayerInfo};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::Sender;
use tokio::sync::{watch, Mutex};
use warp::http::{Response, StatusCode};
use warp::hyper::Body;
use warp::{Filter, Rejection, Reply};

#[derive(serde::Deserialize)]
struct PlaylistRequest {
    id: String,
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

    let status_route = warp::path("status")
        .and(warp::get())
        .and(info_filter.clone())
        .and_then(handle_status);

    playlist_route.or(play_route).or(pause_route).or(next_route).or(status_route).boxed()
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
        .send(PlayerCommand::Playlist(req.id))
        .await
        .expect("Failed to send player command");

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "ok" })),
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
        .expect("Failed to send pause command");

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "success" })),
        StatusCode::OK,
    )
        .into_response())
}
