use anyhow::{Context, Result};
use tracing::{debug, info};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::config::TranscriberConfig;
use crate::util;

pub struct Transcriber {
    ctx: WhisperContext,
    language: String,
}

impl Transcriber {
    pub fn new(config: &TranscriberConfig) -> Result<Self> {
        let model_path = ensure_model(&config.model_size)?;

        info!("Loading whisper model: {}", model_path.display());
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            WhisperContextParameters::default(),
        )
        .context("Failed to load whisper model")?;

        info!("Whisper model loaded: {}", config.model_size);

        Ok(Self {
            ctx,
            language: config.language.clone(),
        })
    }

    pub fn transcribe(&self, audio: &[f32], _sample_rate: u32) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let mut state = self.ctx.create_state().context("Failed to create whisper state")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_single_segment(true);

        state
            .full(params, audio)
            .context("Whisper transcription failed")?;

        let num_segments = state.full_n_segments()?;
        let mut text = String::new();

        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(segment.trim());
                text.push(' ');
            }
        }

        let result = text.trim().to_string();
        if !result.is_empty() {
            debug!("Transcribed: \"{}\"", result);
        }
        Ok(result)
    }
}

fn ensure_model(model_size: &str) -> Result<std::path::PathBuf> {
    let model_file = format!("ggml-{}.bin", model_size);
    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model_file
    );
    util::download_cached(&url, &model_file)
}
