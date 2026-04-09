use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub audio: AudioConfig,
    pub vad: VadConfig,
    pub transcriber: TranscriberConfig,
    pub agreement: AgreementConfig,
    pub injector: InjectorConfig,
    pub daemon: DaemonConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_duration_ms: u32,
    #[serde(default)]
    pub device: Option<String>,
}

impl AudioConfig {
    pub fn frame_samples(&self) -> usize {
        (self.sample_rate * self.frame_duration_ms / 1000) as usize
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct VadConfig {
    pub threshold: f32,
    pub silence_duration_ms: u32,
    pub min_speech_duration_ms: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TranscriberConfig {
    pub model_size: String,
    pub language: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgreementConfig {
    pub n: usize,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Xdotool,
    Ydotool,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Backend::Xdotool => write!(f, "xdotool"),
            Backend::Ydotool => write!(f, "ydotool"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct InjectorConfig {
    pub backend: Backend,
    pub inter_key_delay_ms: u32,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Vox,
    Ptt,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Vox => write!(f, "vox"),
            Mode::Ptt => write!(f, "ptt"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DaemonConfig {
    pub mode: Mode,
    pub pid_file: String,
    pub socket_path: String,
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_str = if let Some(p) = path {
            std::fs::read_to_string(p)
                .with_context(|| format!("Failed to read config: {}", p.display()))?
        } else {
            let user_config = user_config_path();
            if user_config.exists() {
                std::fs::read_to_string(&user_config)?
            } else {
                include_str!("../config/default.yaml").to_string()
            }
        };

        let config: Config =
            serde_yaml::from_str(&config_str).context("Failed to parse config YAML")?;
        Ok(config)
    }
}

fn user_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("whisper-vox")
        .join("config.yaml")
}
