use anyhow::{Context, Result};
use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use tracing::{debug, info};

use crate::config::VadConfig;
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VadEvent {
    Silence,
    SpeechStart,
    SpeechContinue,
    SpeechEnd,
}

pub struct VoiceActivityDetector {
    session: Session,
    config: VadConfig,
    // LSTM state
    h: Array3<f32>,
    c: Array3<f32>,
    // Pre-allocated sample rate tensor (never changes)
    sr: Array1<i64>,
    // Internal state
    is_speaking: bool,
    silence_frames: u32,
    speech_frames: u32,
    frames_per_ms: f32,
}

impl VoiceActivityDetector {
    pub fn new(config: &VadConfig, sample_rate: u32, frame_duration_ms: u32) -> Result<Self> {
        let model_path = util::download_cached(
            "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx",
            "silero_vad.onnx",
        )?;

        let session = Session::builder()?
            .commit_from_file(&model_path)
            .with_context(|| format!("Failed to load VAD model: {}", model_path.display()))?;

        info!("Silero-VAD model loaded from {}", model_path.display());

        let frames_per_ms = 1.0 / frame_duration_ms as f32;

        Ok(Self {
            session,
            config: config.clone(),
            h: Array3::zeros((2, 1, 64)),
            c: Array3::zeros((2, 1, 64)),
            sr: Array1::from_vec(vec![sample_rate as i64]),
            is_speaking: false,
            silence_frames: 0,
            speech_frames: 0,
            frames_per_ms,
        })
    }

    pub fn process_frame(&mut self, frame: &[f32], sample_rate: u32) -> Result<VadEvent> {
        let prob = self.run_inference(frame, sample_rate)?;

        let is_speech = prob > self.config.threshold;
        let silence_threshold_frames =
            (self.config.silence_duration_ms as f32 * self.frames_per_ms) as u32;
        let min_speech_frames =
            (self.config.min_speech_duration_ms as f32 * self.frames_per_ms) as u32;

        let event = if self.is_speaking {
            if is_speech {
                self.silence_frames = 0;
                self.speech_frames += 1;
                VadEvent::SpeechContinue
            } else {
                self.silence_frames += 1;
                if self.silence_frames >= silence_threshold_frames {
                    self.is_speaking = false;
                    let total_speech = self.speech_frames;
                    self.speech_frames = 0;
                    self.silence_frames = 0;
                    if total_speech >= min_speech_frames {
                        debug!("Speech ended after {} frames", total_speech);
                        VadEvent::SpeechEnd
                    } else {
                        debug!("Speech too short ({} frames), discarding", total_speech);
                        VadEvent::Silence
                    }
                } else {
                    VadEvent::SpeechContinue
                }
            }
        } else if is_speech {
            self.is_speaking = true;
            self.speech_frames = 1;
            self.silence_frames = 0;
            debug!("Speech started (prob={:.3})", prob);
            VadEvent::SpeechStart
        } else {
            VadEvent::Silence
        };

        Ok(event)
    }

    fn run_inference(&mut self, frame: &[f32], _sample_rate: u32) -> Result<f32> {
        use ort::value::Value;

        let audio = Array2::from_shape_vec((1, frame.len()), frame.to_vec())?;

        // Take ownership of h/c to avoid clone, replace with zeros temporarily
        let h = std::mem::replace(&mut self.h, Array3::zeros((2, 1, 64)));
        let c = std::mem::replace(&mut self.c, Array3::zeros((2, 1, 64)));

        let audio_value = Value::from_array(audio)?;
        let h_value = Value::from_array(h)?;
        let c_value = Value::from_array(c)?;
        let sr_value = Value::from_array(self.sr.clone())?;

        let inputs = vec![
            (std::borrow::Cow::Borrowed("input"), ort::session::SessionInputValue::from(audio_value)),
            (std::borrow::Cow::Borrowed("h"), ort::session::SessionInputValue::from(h_value)),
            (std::borrow::Cow::Borrowed("c"), ort::session::SessionInputValue::from(c_value)),
            (std::borrow::Cow::Borrowed("sr"), ort::session::SessionInputValue::from(sr_value)),
        ];

        let outputs = self.session.run(inputs)?;

        let prob_tensor = outputs[0].try_extract_tensor::<f32>()?;
        let prob = prob_tensor.1[0];

        // Update LSTM state from output tensors
        let (_, h_data) = outputs[1].try_extract_tensor::<f32>()?;
        let (_, c_data) = outputs[2].try_extract_tensor::<f32>()?;
        self.h = Array3::from_shape_vec((2, 1, 64), h_data.to_vec())?;
        self.c = Array3::from_shape_vec((2, 1, 64), c_data.to_vec())?;

        Ok(prob)
    }

    pub fn reset_state(&mut self) {
        self.h = Array3::zeros((2, 1, 64));
        self.c = Array3::zeros((2, 1, 64));
        self.is_speaking = false;
        self.silence_frames = 0;
        self.speech_frames = 0;
    }
}
