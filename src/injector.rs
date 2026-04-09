use anyhow::Result;
use std::process::Command;
use tracing::{debug, warn};

use crate::config::{Backend, InjectorConfig};

pub struct TextInjector {
    backend: Backend,
    inter_key_delay_ms: u32,
}

impl TextInjector {
    pub fn new(config: &InjectorConfig) -> Result<Self> {
        let injector = Self {
            backend: config.backend,
            inter_key_delay_ms: config.inter_key_delay_ms,
        };

        if !injector.is_available() {
            warn!(
                "'{}' not found in PATH. Install it: sudo apt install {}",
                config.backend, config.backend
            );
        }

        Ok(injector)
    }

    pub fn inject(&self, text: &str) -> Result<bool> {
        if text.is_empty() {
            return Ok(true);
        }

        debug!("Injecting text: \"{}\"", text);

        let backend_str = self.backend.to_string();
        let delay = self.inter_key_delay_ms.to_string();

        let result = match self.backend {
            Backend::Xdotool => Command::new("xdotool")
                .args(["type", "--delay", &delay, "--", text])
                .output(),
            Backend::Ydotool => Command::new("ydotool")
                .args(["type", "--key-delay", &delay, "--", text])
                .output(),
        };

        match result {
            Ok(output) if output.status.success() => Ok(true),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("{} failed: {}", backend_str, stderr);
                Ok(false)
            }
            Err(e) => {
                warn!("{} execution failed: {}", backend_str, e);
                Ok(false)
            }
        }
    }

    pub fn is_available(&self) -> bool {
        which::which(self.backend.to_string()).is_ok()
    }
}
