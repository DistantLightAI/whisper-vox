use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use tracing::{debug, error, info};

use crate::config::AudioConfig;

pub struct AudioCapture {
    config: AudioConfig,
    frame_samples: usize,
    rx: Option<mpsc::Receiver<Vec<f32>>>,
    _stream: Option<Stream>,
    paused: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

impl AudioCapture {
    pub fn new(config: &AudioConfig) -> Result<Self> {
        let frame_samples = config.frame_samples();

        Ok(Self {
            config: config.clone(),
            frame_samples,
            rx: None,
            _stream: None,
            paused: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();

        let device = if let Some(ref name) = self.config.device {
            host.input_devices()?
                .find(|d| d.name().map(|n| n == *name).unwrap_or(false))
                .with_context(|| format!("Audio device '{}' not found", name))?
        } else {
            host.default_input_device()
                .context("No default input device found")?
        };

        info!("Using audio device: {}", device.name().unwrap_or_default());

        let stream_config = StreamConfig {
            channels: self.config.channels,
            sample_rate: SampleRate(self.config.sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);
        let paused = self.paused.clone();
        let running = self.running.clone();
        let frame_samples = self.frame_samples;

        // Accumulate samples and emit complete frames
        let mut buffer = Vec::with_capacity(frame_samples * 2);

        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                if !running.load(Ordering::Relaxed) || paused.load(Ordering::Relaxed) {
                    return;
                }

                buffer.extend_from_slice(data);

                while buffer.len() >= frame_samples {
                    let frame: Vec<f32> = buffer.drain(..frame_samples).collect();
                    if tx.try_send(frame).is_err() {
                        debug!("Frame queue full, dropping frame");
                    }
                }
            },
            move |err| {
                error!("Audio stream error: {}", err);
            },
            None,
        )?;

        stream.play()?;
        self.running.store(true, Ordering::Relaxed);
        self._stream = Some(stream);
        self.rx = Some(rx);

        info!("Audio capture started ({}Hz, {} channels, {}ms frames)",
              self.config.sample_rate, self.config.channels, self.config.frame_duration_ms);

        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        self._stream = None;
        self.rx = None;
        info!("Audio capture stopped");
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
        info!("Audio capture paused (PTT mode)");
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
        info!("Audio capture resumed (VOX mode)");
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn try_read_frame(&self) -> Option<Vec<f32>> {
        self.rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
    }
}
