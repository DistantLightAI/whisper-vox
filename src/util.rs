use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

/// Shared cache directory for whisper-vox models.
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("whisper-vox")
}

/// Download a file to the cache directory if not already present.
pub fn download_cached(url: &str, filename: &str) -> Result<PathBuf> {
    let dir = cache_dir();
    let path = dir.join(filename);

    if path.exists() {
        return Ok(path);
    }

    std::fs::create_dir_all(&dir)?;
    info!("Downloading {}...", filename);

    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    let bytes = response.bytes()?;
    std::fs::write(&path, &bytes)?;

    info!("Saved to {}", path.display());
    Ok(path)
}
