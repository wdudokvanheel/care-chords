use std::env;
use anyhow::{Result};
use clap::{Parser};
use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ApplicationSettings {
    /// The target RTSP server URL
    pub rtsp_server: String,
    pub monitor_url: String,
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "CareChords Server",
    long_about = "The CareChords Server streams your IP camera combined with the integrated Spotify client to an RTSP server, which can be consumed by the accompanying CareChords app."
)]
struct Cli {
    #[arg(short = 't', long = "rtsp-server", help = "Set the target RTSP server URL")]
    rtsp_server: Option<String>,
    #[arg(short = 'm', long = "monitor-url", help = "Set the URL of the baby monitor")]
    monitor_url: Option<String>,
}

impl ApplicationSettings {
    pub(crate) fn load() -> Result<Self> {
        let mut config_builder = Config::builder();

        // Check for a user-specified config file from the CARECHORDS_CONF environment variable
        if let Ok(custom_conf) = env::var("CARECHORDS_CONF") {
            let path = Path::new(&custom_conf);
            if path.exists() {
                config_builder = config_builder.add_source(File::with_name(&custom_conf).required(true));
            } else {
                anyhow::bail!("The configuration file specified in CARECHORDS_CONF does not exist: {}", custom_conf);
            }
        } else {
            // Search in standard locations if CARECHORDS_CONF is not set
            let config_paths = [
                "/etc/carechords.toml",
                "/usr/local/etc/carechords.toml",
                "/opt/carechords/carechords.toml",
            ];

            for path in &config_paths {
                if Path::new(path).exists() {
                    config_builder = config_builder.add_source(File::with_name(path).required(false));
                }
            }
        }

        // Load environment variables prefixed with CARECHORDS_
        config_builder = config_builder.add_source(Environment::with_prefix("CARECHORDS"));

        // Build configuration
        let mut settings: ApplicationSettings = config_builder.build()?.try_deserialize()?;

        // Override with CLI arguments if provided
        let cli = Cli::parse();
        if let Some(rtsp_server) = cli.rtsp_server {
            settings.rtsp_server = rtsp_server;
        }
        if let Some(monitor_url) = cli.monitor_url {
            settings.monitor_url = monitor_url;
        }

        // Display resolved configuration if requested
        log::info!("Running with settings: {:#?}", settings);

        Ok(settings)
    }
}
