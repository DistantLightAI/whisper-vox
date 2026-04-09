mod agreement;
mod audio;
mod config;
mod daemon;
mod injector;
mod sentence;
mod transcriber;
mod util;
mod vad;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::info;

use crate::config::Config;
use crate::daemon::WhisperVoxDaemon;

#[derive(Parser)]
#[command(name = "whisper-vox", about = "Always-listening voice daemon for Claude Code")]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon (default)
    Start,
    /// Toggle between VOX and PTT modes
    Toggle,
    /// Show daemon status
    Status,
    /// Stop the daemon
    Stop,
    /// Install systemd user service
    Install,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("whisper_vox=info".parse()?),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let config_path = cli.config.as_deref();

    match cli.command.unwrap_or(Commands::Start) {
        Commands::Start => {
            let config = Config::load(config_path)?;
            let mut daemon = WhisperVoxDaemon::new(config);
            daemon.run().await?;
        }
        Commands::Toggle => {
            send_ipc_command("toggle", config_path).await?;
        }
        Commands::Status => {
            send_ipc_command("status", config_path).await?;
        }
        Commands::Stop => {
            send_ipc_command("stop", config_path).await?;
        }
        Commands::Install => {
            install_service()?;
        }
    }

    Ok(())
}

async fn send_ipc_command(cmd: &str, config_path: Option<&std::path::Path>) -> Result<()> {
    let config = Config::load(config_path)?;
    let socket_path = &config.daemon.socket_path;

    let stream = UnixStream::connect(socket_path).await
        .map_err(|_| anyhow::anyhow!("Daemon not running (can't connect to {})", socket_path))?;

    let (reader, mut writer) = stream.into_split();

    let request = serde_json::json!({"cmd": cmd});
    writer
        .write_all(format!("{}\n", request).as_bytes())
        .await?;

    let mut reader = BufReader::new(reader);
    let mut response = String::new();
    reader.read_line(&mut response).await?;

    let resp: serde_json::Value = serde_json::from_str(response.trim())?;

    match cmd {
        "status" => {
            println!("Mode: {}", resp["mode"].as_str().unwrap_or("unknown"));
            println!("Uptime: {}s", resp["uptime_secs"].as_u64().unwrap_or(0));
            println!("Running: {}", resp["running"].as_bool().unwrap_or(false));
        }
        "toggle" => {
            println!("Toggled. {}", resp);
        }
        "stop" => {
            println!("Stopping daemon...");
        }
        _ => {
            println!("{}", resp);
        }
    }

    Ok(())
}

fn install_service() -> Result<()> {
    let exe_path = std::env::current_exe()?;
    let service_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("systemd/user");
    std::fs::create_dir_all(&service_dir)?;

    let service_path = service_dir.join("whisper-vox.service");
    let service_content = format!(
        r#"[Unit]
Description=whisper-vox always-listening voice daemon
After=pipewire.service

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=3
Environment=DISPLAY=:0

[Install]
WantedBy=default.target
"#,
        exe_path.display()
    );

    std::fs::write(&service_path, service_content)?;
    info!("Service file written to {}", service_path.display());

    println!("Service installed at {}", service_path.display());
    println!("Enable with: systemctl --user enable whisper-vox");
    println!("Start with:  systemctl --user start whisper-vox");

    Ok(())
}
