mod app_settings;
mod pipeline;
mod server;
mod spotify_client;
mod spotify_player;
mod spotify_sink;
mod webserver;

use crate::app_settings::ApplicationSettings;
use crate::server::CareChordsServer;
use anyhow::{Context, Error};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Error> {
    setup_logging()?;

    let settings = ApplicationSettings::load().context("Failed to load settings")?;
    let mut server = CareChordsServer::new(&settings);
    let rtsp_server = crate::pipeline::rtsp_server::RtspServer::new(settings.rtsp_port)?;

    tokio::spawn(async move {
        if let Err(e) = rtsp_server.start().await {
            log::error!("RTSP server error: {}", e);
        }
    });

    server.start().await;

    // Keep the runtime alive
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");

    Ok(())
}

fn setup_logging() -> Result<(), Error> {
    simple_logger::SimpleLogger::new()
        .with_colors(true)
        .with_module_level("warp", log::LevelFilter::Warn)
        .with_module_level("hyper", log::LevelFilter::Warn)
        .with_module_level("libmdns", log::LevelFilter::Warn)
        .with_module_level("rustls", log::LevelFilter::Warn)
        .with_module_level("symphonia_format_ogg", log::LevelFilter::Warn)
        .with_module_level("librespot", log::LevelFilter::Warn)
        .with_module_level("librespot_core", log::LevelFilter::Warn)
        .with_module_level("h2", log::LevelFilter::Warn)
        .with_level(log::LevelFilter::Debug)
        .init()?;

    Ok(())
}
