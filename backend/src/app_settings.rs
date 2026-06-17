use crate::data_paths;
use anyhow::Result;
use clap::Parser;
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApplicationSettings {
    /// The folder containing credentials.json, cache/, and music/
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    /// The target RTSP server port
    #[serde(default = "default_rtsp_port")]
    pub rtsp_port: u16,
    pub monitor_url: String,
    /// Enable noise filtering
    #[serde(default)]
    pub noise_filter: bool,
    #[serde(default)]
    pub local_audio: LocalAudioSettings,
    #[serde(default)]
    pub youtube_audio: LocalAudioSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalAudioSettings {
    #[serde(default = "default_local_roots")]
    pub roots: Vec<String>,
    #[serde(default = "default_allowed_extensions")]
    pub allowed_extensions: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigSettings {
    data_dir: Option<String>,
    rtsp_port: Option<u16>,
    monitor_url: Option<String>,
    #[serde(default)]
    noise_filter: bool,
    #[serde(default)]
    local_audio: Option<LocalAudioSettings>,
    #[serde(default)]
    youtube_audio: Option<LocalAudioSettings>,
}

fn default_rtsp_port() -> u16 {
    8554
}

fn default_data_dir() -> String {
    ".".to_string()
}

fn default_local_roots() -> Vec<String> {
    vec!["music".to_string()]
}

fn default_allowed_extensions() -> Vec<String> {
    [
        "mp3", "flac", "m4a", "mp4", "aac", "ogg", "opus", "wav", "webm",
    ]
    .iter()
    .map(|ext| ext.to_string())
    .collect()
}

impl Default for LocalAudioSettings {
    fn default() -> Self {
        Self::for_data_dir(default_data_dir())
    }
}

impl LocalAudioSettings {
    fn for_data_dir(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![data_dir.into().join("music").to_string_lossy().to_string()],
            allowed_extensions: default_allowed_extensions(),
        }
    }

    fn for_youtube_dir(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![
                data_dir
                    .into()
                    .join("youtube")
                    .to_string_lossy()
                    .to_string(),
            ],
            allowed_extensions: default_allowed_extensions(),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "CareChords Server",
    long_about = "The CareChords Server streams your IP camera combined with the integrated Spotify client to an RTSP server, which can be consumed by the accompanying CareChords app."
)]
struct Cli {
    #[arg(
        short = 'p',
        long = "rtsp-port",
        help = "Set the target RTSP server port"
    )]
    rtsp_port: Option<u16>,
    #[arg(
        short = 'm',
        long = "monitor-url",
        help = "Set the URL of the baby monitor"
    )]
    monitor_url: Option<String>,
    #[arg(long = "noise-filter", help = "Enable noise filtering")]
    noise_filter: bool,
    #[arg(
        long = "data-dir",
        help = "Set the folder containing credentials.json, cache/, and music/"
    )]
    data_dir: Option<String>,
}

impl ApplicationSettings {
    pub(crate) fn load() -> Result<Self> {
        Self::load_with_cli(Cli::parse())
    }

    fn load_with_cli(cli: Cli) -> Result<Self> {
        let mut config_builder = Config::builder();

        // Check for a user-specified config file from the CARECHORDS_CONF environment variable
        if let Ok(custom_conf) = env::var("CARECHORDS_CONF") {
            let path = Path::new(&custom_conf);
            if path.exists() {
                config_builder =
                    config_builder.add_source(File::with_name(&custom_conf).required(true));
            } else {
                anyhow::bail!(
                    "The configuration file specified in CARECHORDS_CONF does not exist: {}",
                    custom_conf
                );
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
                    config_builder =
                        config_builder.add_source(File::with_name(path).required(false));
                }
            }
        }

        // Load environment variables prefixed with CARECHORDS_
        config_builder = config_builder.add_source(Environment::with_prefix("CARECHORDS"));

        // Build configuration. Required values may still be supplied by CLI flags,
        // so deserialize into an optional representation first.
        let loaded_settings: ConfigSettings = config_builder.build()?.try_deserialize()?;
        let data_dir = loaded_settings.data_dir.unwrap_or_else(default_data_dir);
        let local_audio_configured = loaded_settings.local_audio.is_some();
        let youtube_audio_configured = loaded_settings.youtube_audio.is_some();
        let mut settings = ApplicationSettings {
            data_dir: data_dir.clone(),
            rtsp_port: loaded_settings.rtsp_port.unwrap_or_else(default_rtsp_port),
            monitor_url: loaded_settings.monitor_url.unwrap_or_default(),
            noise_filter: loaded_settings.noise_filter,
            local_audio: loaded_settings
                .local_audio
                .unwrap_or_else(|| LocalAudioSettings::for_data_dir(&data_dir)),
            youtube_audio: loaded_settings
                .youtube_audio
                .unwrap_or_else(|| LocalAudioSettings::for_youtube_dir(&data_dir)),
        };

        // Override with CLI arguments if provided
        if let Some(data_dir) = cli.data_dir {
            settings.data_dir = data_dir;
            if !local_audio_configured {
                settings.local_audio = LocalAudioSettings::for_data_dir(&settings.data_dir);
            }
            if !youtube_audio_configured {
                settings.youtube_audio = LocalAudioSettings::for_youtube_dir(&settings.data_dir);
            }
        }
        if let Some(rtsp_port) = cli.rtsp_port {
            settings.rtsp_port = rtsp_port;
        }
        if let Some(monitor_url) = cli.monitor_url {
            settings.monitor_url = monitor_url;
        }
        // Only override noise_filter if the CLI flag is activated (true)
        if cli.noise_filter {
            settings.noise_filter = true;
        }

        if settings.monitor_url.is_empty() {
            anyhow::bail!(
                "monitor_url is required; set it in carechords.toml, CARECHORDS_MONITOR_URL, or --monitor-url"
            );
        }

        data_paths::set_data_dir(&settings.data_dir);

        let toml_str =
            toml::to_string_pretty(&settings).expect("Failed to convert settings to TOML format");
        log::info!("Running with settings:\n{}", toml_str);
        Ok(settings)
    }
}
