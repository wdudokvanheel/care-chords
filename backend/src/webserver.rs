use crate::playback_controller::{LegacyPlaylistRequest, PlayRefRequest, PlaybackController};
use crate::system_playlists::{AddSystemPlaylistItemRequest, CreateSystemPlaylistRequest};
use futures_util::StreamExt;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::watch;
use warp::http::{Response, StatusCode, header};
use warp::hyper::Body;
use warp::{Filter, Rejection, Reply};

#[derive(Deserialize)]
struct SleepTimerRequest {
    timer: u32,
}

#[derive(Deserialize)]
struct ShuffleRequest {
    shuffle: bool,
}

#[derive(Deserialize)]
struct LocalLibraryQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
struct LocalArtworkQuery {
    path: String,
}

pub fn start_http_server(playback: Arc<PlaybackController>, monitor_url: String) {
    log::info!("Starting server @ :7755");
    let routes = create_routes(playback, monitor_url);
    tokio::spawn(async move {
        warp::serve(routes).run(([0, 0, 0, 0], 7755)).await;
    });
}

fn create_routes(
    playback: Arc<PlaybackController>,
    monitor_url: String,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let playback_filter = warp::any().map(move || playback.clone());
    let monitor_url_filter = warp::any().map(move || monitor_url.clone());

    let sources_route = warp::path("sources")
        .and(warp::path::end())
        .and(warp::get())
        .and(playback_filter.clone())
        .and_then(handle_sources);

    let local_library_route = warp::path!("library" / "local")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::query::<LocalLibraryQuery>())
        .and(playback_filter.clone())
        .and_then(handle_local_library);

    let local_artwork_route = warp::path!("library" / "local" / "artwork")
        .and(warp::get())
        .and(warp::query::<LocalArtworkQuery>())
        .and(playback_filter.clone())
        .and_then(handle_local_artwork);

    let youtube_library_route = warp::path!("library" / "youtube")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::query::<LocalLibraryQuery>())
        .and(playback_filter.clone())
        .and_then(handle_youtube_library);

    let youtube_artwork_route = warp::path!("library" / "youtube" / "artwork")
        .and(warp::get())
        .and(warp::query::<LocalArtworkQuery>())
        .and(playback_filter.clone())
        .and_then(handle_youtube_artwork);

    let system_playlists_route = warp::path("system-playlists")
        .and(warp::path::end())
        .and(warp::get())
        .and(playback_filter.clone())
        .and_then(handle_system_playlists);

    let create_system_playlist_route = warp::path("system-playlists")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json::<CreateSystemPlaylistRequest>())
        .and(playback_filter.clone())
        .and_then(handle_create_system_playlist);

    let add_system_playlist_item_route = warp::path!("system-playlists" / String / "items")
        .and(warp::post())
        .and(warp::body::json::<AddSystemPlaylistItemRequest>())
        .and(playback_filter.clone())
        .and_then(handle_add_system_playlist_item);

    let play_ref_route = warp::path!("queue" / "play-ref")
        .and(warp::post())
        .and(warp::body::json::<PlayRefRequest>())
        .and(playback_filter.clone())
        .and_then(handle_play_ref);

    let play_system_playlist_route = warp::path!("queue" / "play-system-playlist" / String)
        .and(warp::post())
        .and(playback_filter.clone())
        .and_then(handle_play_system_playlist);

    let playlist_route = warp::path("playlist")
        .and(warp::post())
        .and(warp::body::json::<LegacyPlaylistRequest>())
        .and(playback_filter.clone())
        .and_then(handle_legacy_playlist);

    let playlists_route = warp::path("playlists")
        .and(warp::get())
        .and(playback_filter.clone())
        .and_then(handle_spotify_playlists);

    let play_route = warp::path("play")
        .and(warp::post())
        .and(playback_filter.clone())
        .and_then(handle_play);

    let pause_route = warp::path("pause")
        .and(warp::post())
        .and(playback_filter.clone())
        .and_then(handle_pause);

    let next_route = warp::path("next")
        .and(warp::post())
        .and(playback_filter.clone())
        .and_then(handle_next);

    let sleep_route = warp::path("sleep")
        .and(warp::post())
        .and(warp::body::json::<SleepTimerRequest>())
        .and(playback_filter.clone())
        .and_then(handle_sleep);

    let shuffle_route = warp::path("shuffle")
        .and(warp::post())
        .and(warp::body::json::<ShuffleRequest>())
        .and(playback_filter.clone())
        .and_then(handle_shuffle);

    let status_route = warp::path("status")
        .and(warp::get())
        .and(playback_filter.clone())
        .and_then(handle_status);

    let audio_status_route = warp::path!("audio" / "status")
        .and(warp::get())
        .and(playback_filter.clone())
        .and_then(handle_status);

    let monitor_route = warp::path("monitor")
        .and(warp::get())
        .and(monitor_url_filter)
        .and_then(handle_monitor);

    let status_stream_route = warp::path("status_stream")
        .and(warp::get())
        .and(playback_filter.clone())
        .map(status_stream_reply);

    let audio_status_stream_route = warp::path!("audio" / "status_stream")
        .and(warp::get())
        .and(playback_filter)
        .map(status_stream_reply);

    sources_route
        .or(local_library_route)
        .or(local_artwork_route)
        .or(youtube_library_route)
        .or(youtube_artwork_route)
        .or(system_playlists_route)
        .or(create_system_playlist_route)
        .or(add_system_playlist_item_route)
        .or(play_ref_route)
        .or(play_system_playlist_route)
        .or(playlist_route)
        .or(playlists_route)
        .or(play_route)
        .or(pause_route)
        .or(next_route)
        .or(status_route)
        .or(audio_status_route)
        .or(sleep_route)
        .or(shuffle_route)
        .or(monitor_route)
        .or(status_stream_route)
        .or(audio_status_stream_route)
        .boxed()
}

async fn handle_sources(playback: Arc<PlaybackController>) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&playback.sources()))
}

async fn handle_local_library(
    query: LocalLibraryQuery,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.local_library().list(query.path.as_deref()) {
        Ok(entries) => Ok(no_store(json_status(&entries, StatusCode::OK))),
        Err(e) => Ok(no_store(error_status(
            &e.to_string(),
            StatusCode::BAD_REQUEST,
        ))),
    }
}

async fn handle_youtube_library(
    query: LocalLibraryQuery,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.youtube_library().list(query.path.as_deref()) {
        Ok(entries) => Ok(no_store(json_status(&entries, StatusCode::OK))),
        Err(e) => Ok(no_store(error_status(
            &e.to_string(),
            StatusCode::BAD_REQUEST,
        ))),
    }
}

async fn handle_local_artwork(
    query: LocalArtworkQuery,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    let path = match playback.local_library().resolve_artwork_ref(&query.path) {
        Ok(path) => path,
        Err(e) => {
            return Ok(no_store(error_status(
                &e.to_string(),
                StatusCode::NOT_FOUND,
            )));
        }
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => Ok(no_store(image_status(bytes, image_content_type(&path)))),
        Err(e) => Ok(no_store(error_status(
            &format!("Failed to read local audio artwork: {e}"),
            StatusCode::NOT_FOUND,
        ))),
    }
}

async fn handle_youtube_artwork(
    query: LocalArtworkQuery,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    let path = match playback.youtube_library().resolve_artwork_ref(&query.path) {
        Ok(path) => path,
        Err(e) => {
            return Ok(no_store(error_status(
                &e.to_string(),
                StatusCode::NOT_FOUND,
            )));
        }
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => Ok(no_store(image_status(bytes, image_content_type(&path)))),
        Err(e) => Ok(no_store(error_status(
            &format!("Failed to read YouTube audio artwork: {e}"),
            StatusCode::NOT_FOUND,
        ))),
    }
}

async fn handle_system_playlists(
    playback: Arc<PlaybackController>,
) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&playback.system_playlists().list()))
}

async fn handle_create_system_playlist(
    req: CreateSystemPlaylistRequest,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.system_playlists().create(req.name) {
        Ok(playlist) => Ok(json_status(&playlist, StatusCode::CREATED)),
        Err(e) => Ok(error_status(
            &e.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn handle_add_system_playlist_item(
    playlist_id: String,
    req: AddSystemPlaylistItemRequest,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.system_playlists().add_item(&playlist_id, req) {
        Ok(playlist) => Ok(json_status(&playlist, StatusCode::OK)),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

async fn handle_play_ref(
    req: PlayRefRequest,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.play_ref(&req.reference).await {
        Ok(()) => Ok(ok_status("playing")),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

async fn handle_play_system_playlist(
    playlist_id: String,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.play_system_playlist(&playlist_id).await {
        Ok(()) => Ok(ok_status("playing")),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

async fn handle_legacy_playlist(
    req: LegacyPlaylistRequest,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.play_ref(&req.uri).await {
        Ok(()) => Ok(ok_status("ok")),
        Err(e) => Ok(error_status(
            &e.to_string(),
            StatusCode::SERVICE_UNAVAILABLE,
        )),
    }
}

async fn handle_spotify_playlists(
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.spotify_playlists().await {
        Ok(playlists) => Ok(json_status(&playlists, StatusCode::OK)),
        Err(e) => {
            log::warn!("Failed to fetch Spotify playlists: {e}");
            Ok(error_status(
                &e.to_string(),
                StatusCode::SERVICE_UNAVAILABLE,
            ))
        }
    }
}

async fn handle_status(playback: Arc<PlaybackController>) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&playback.current_info()))
}

async fn handle_monitor(monitor_url: String) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&serde_json::json!({
        "url": monitor_url
    })))
}

async fn handle_sleep(
    req: SleepTimerRequest,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.sleep(req.timer).await {
        Ok(()) => Ok(json_status(&playback.current_info(), StatusCode::OK)),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

async fn handle_shuffle(
    req: ShuffleRequest,
    playback: Arc<PlaybackController>,
) -> Result<Response<Body>, Rejection> {
    match playback.shuffle(req.shuffle).await {
        Ok(info) => Ok(json_status(&info, StatusCode::OK)),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

async fn handle_play(playback: Arc<PlaybackController>) -> Result<Response<Body>, Rejection> {
    match playback.play().await {
        Ok(()) => Ok(json_status(&playback.current_info(), StatusCode::OK)),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

async fn handle_pause(playback: Arc<PlaybackController>) -> Result<Response<Body>, Rejection> {
    match playback.pause().await {
        Ok(()) => Ok(json_status(&playback.current_info(), StatusCode::OK)),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

async fn handle_next(playback: Arc<PlaybackController>) -> Result<Response<Body>, Rejection> {
    match playback.next().await {
        Ok(()) => Ok(json_status(&playback.current_info(), StatusCode::OK)),
        Err(e) => Ok(error_status(&e.to_string(), StatusCode::BAD_REQUEST)),
    }
}

fn status_stream_reply(playback: Arc<PlaybackController>) -> impl Reply {
    let mut info_channel: watch::Receiver<_> = playback.info_channel();
    let event_stream: futures_util::stream::BoxStream<
        'static,
        Result<warp::sse::Event, std::convert::Infallible>,
    > = async_stream::stream! {
        let initial_state = info_channel.borrow().clone();
        let mut last_emitted = serde_json::to_string(&initial_state)
            .unwrap_or_else(|_| "{}".to_string());
        yield Ok(warp::sse::Event::default().data(last_emitted.clone()));

        loop {
            if info_channel.changed().await.is_err() {
                break;
            }
            let current_state = info_channel.borrow().clone();
            let current_json = serde_json::to_string(&current_state)
                .unwrap_or_else(|_| "{}".to_string());
            if current_json != last_emitted {
                last_emitted = current_json.clone();
                yield Ok(warp::sse::Event::default().data(current_json));
            }
        }
    }
    .boxed();

    warp::sse::reply(warp::sse::keep_alive().stream(event_stream))
}

fn ok_status(status: &str) -> Response<Body> {
    json_status(&serde_json::json!({ "status": status }), StatusCode::OK)
}

fn error_status(message: &str, status: StatusCode) -> Response<Body> {
    json_status(&serde_json::json!({ "error": message }), status)
}

fn json_status<T: serde::Serialize>(body: &T, status: StatusCode) -> Response<Body> {
    warp::reply::with_status(warp::reply::json(body), status).into_response()
}

fn image_status(bytes: Vec<u8>, content_type: &'static str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(bytes))
        .unwrap()
}

fn image_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

fn no_store(mut response: Response<Body>) -> Response<Body> {
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        "no-store, no-cache, must-revalidate".parse().unwrap(),
    );
    headers.insert(header::PRAGMA, "no-cache".parse().unwrap());
    headers.insert(header::EXPIRES, "0".parse().unwrap());
    response
}
