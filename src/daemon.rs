use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tracing::{debug, error, info, warn};

use crate::agreement::LocalAgreement;
use crate::audio::AudioCapture;
use crate::config::{Config, Mode};
use crate::injector::TextInjector;
use crate::sentence::SentenceDetector;
use crate::transcriber::Transcriber;
use crate::vad::{VadEvent, VoiceActivityDetector};

pub struct WhisperVoxDaemon {
    config: Config,
    mode: Mode,
    running: Arc<AtomicBool>,
    start_time: Instant,
}

impl WhisperVoxDaemon {
    pub fn new(config: Config) -> Self {
        let mode = config.daemon.mode;
        Self {
            running: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            mode,
            config,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.running.store(true, Ordering::Relaxed);
        self.write_pid()?;

        info!("whisper-vox daemon starting in {} mode", self.mode);

        let mut audio = AudioCapture::new(&self.config.audio)?;
        let mut vad = VoiceActivityDetector::new(
            &self.config.vad,
            self.config.audio.sample_rate,
            self.config.audio.frame_duration_ms,
        )?;
        let transcriber = Transcriber::new(&self.config.transcriber)?;
        let mut agreement = LocalAgreement::new(self.config.agreement.n);
        let mut sentence_detector = SentenceDetector::new();
        let injector = TextInjector::new(&self.config.injector)?;

        if self.mode == Mode::Vox {
            audio.start()?;
        } else {
            info!("Starting in PTT mode — audio capture paused");
        }

        let running = self.running.clone();
        let socket_path = self.config.daemon.socket_path.clone();
        let ipc_running = running.clone();

        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<IpcCommand>(8);

        tokio::spawn(async move {
            if let Err(e) = run_ipc_server(&socket_path, cmd_tx, ipc_running).await {
                error!("IPC server error: {}", e);
            }
        });

        let mut speech_buffer: Vec<f32> = Vec::new();
        let sample_rate = self.config.audio.sample_rate;

        info!("whisper-vox daemon running");

        loop {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }

            if let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    IpcCommand::Toggle => {
                        self.mode = match self.mode {
                            Mode::Vox => {
                                audio.pause();
                                speech_buffer.clear();
                                agreement.reset();
                                sentence_detector.reset();
                                vad.reset_state();
                                Mode::Ptt
                            }
                            Mode::Ptt => {
                                if audio.is_paused() {
                                    audio.resume();
                                } else {
                                    audio.start()?;
                                }
                                Mode::Vox
                            }
                        };
                        info!("Mode toggled to: {}", self.mode);
                    }
                    IpcCommand::Stop => {
                        info!("Stop command received");
                        self.running.store(false, Ordering::Relaxed);
                        break;
                    }
                    IpcCommand::Status(tx) => {
                        let status = serde_json::json!({
                            "ok": true,
                            "mode": self.mode.to_string(),
                            "uptime_secs": self.start_time.elapsed().as_secs(),
                            "running": true,
                        });
                        let _ = tx.send(status.to_string());
                    }
                }
            }

            if self.mode == Mode::Ptt {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                continue;
            }

            // Use try_read_frame to avoid blocking the tokio runtime
            let frame = match audio.try_read_frame() {
                Some(f) => f,
                None => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                    continue;
                }
            };

            let event = match vad.process_frame(&frame, sample_rate) {
                Ok(e) => e,
                Err(e) => {
                    warn!("VAD error: {}", e);
                    continue;
                }
            };

            match event {
                VadEvent::SpeechStart => {
                    speech_buffer.clear();
                    speech_buffer.extend_from_slice(&frame);
                }
                VadEvent::SpeechContinue => {
                    speech_buffer.extend_from_slice(&frame);
                }
                VadEvent::SpeechEnd => {
                    if speech_buffer.is_empty() {
                        continue;
                    }

                    match transcriber.transcribe(&speech_buffer, sample_rate) {
                        Ok(text) if !text.is_empty() => {
                            if let Some(confirmed) = agreement.process(&text) {
                                let (sentences, _remaining) = sentence_detector.process(&confirmed);
                                for sentence in sentences {
                                    if let Err(e) = injector.inject(&format!("{} ", sentence)) {
                                        warn!("Injection failed: {}", e);
                                    }
                                }
                            }
                        }
                        Ok(_) => debug!("Empty transcription, skipping"),
                        Err(e) => warn!("Transcription error: {}", e),
                    }

                    if let Some(remaining) = sentence_detector.flush() {
                        if let Err(e) = injector.inject(&format!("{} ", remaining)) {
                            warn!("Injection failed: {}", e);
                        }
                    }

                    speech_buffer.clear();
                    agreement.reset();
                    sentence_detector.reset();
                    vad.reset_state();
                }
                VadEvent::Silence => {}
            }
        }

        audio.stop();
        self.remove_pid();
        let _ = std::fs::remove_file(&self.config.daemon.socket_path);
        info!("whisper-vox daemon stopped");

        Ok(())
    }

    fn write_pid(&self) -> Result<()> {
        let pid = std::process::id();
        std::fs::write(&self.config.daemon.pid_file, pid.to_string())
            .context("Failed to write PID file")?;
        Ok(())
    }

    fn remove_pid(&self) {
        let _ = std::fs::remove_file(&self.config.daemon.pid_file);
    }
}

enum IpcCommand {
    Toggle,
    Stop,
    Status(tokio::sync::oneshot::Sender<String>),
}

async fn run_ipc_server(
    socket_path: &str,
    cmd_tx: tokio::sync::mpsc::Sender<IpcCommand>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("Failed to bind Unix socket: {}", socket_path))?;

    info!("IPC server listening on {}", socket_path);

    while running.load(Ordering::Relaxed) {
        let (stream, _) = listener.accept().await?;
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        if reader.read_line(&mut line).await? == 0 {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let cmd = request["cmd"].as_str().unwrap_or("");
        let resp = match cmd {
            "toggle" => {
                let _ = cmd_tx.send(IpcCommand::Toggle).await;
                serde_json::json!({"ok": true, "action": "toggled"})
            }
            "stop" => {
                let _ = cmd_tx.send(IpcCommand::Stop).await;
                serde_json::json!({"ok": true, "action": "stopping"})
            }
            "status" => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let _ = cmd_tx.send(IpcCommand::Status(tx)).await;
                if let Ok(status) = rx.await {
                    serde_json::from_str(&status).unwrap_or(serde_json::json!({"ok": false}))
                } else {
                    serde_json::json!({"ok": false, "error": "timeout"})
                }
            }
            _ => serde_json::json!({"ok": false, "error": "unknown command"}),
        };

        let _ = writer.write_all(format!("{}\n", resp).as_bytes()).await;
    }

    Ok(())
}
