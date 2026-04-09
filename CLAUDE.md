# whisper-vox

Always-listening voice daemon for Claude Code. Replaces hold-spacebar push-to-talk with continuous listening + sentence debouncing.

## Stack
- **Rust** — compiled, zero-GC, thread-safe daemon
- **whisper-rs** — Rust bindings to whisper.cpp, CPU int8 inference
- **ort** — ONNX Runtime Rust bindings for Silero-VAD
- **cpal** — cross-platform audio capture (PipeWire/ALSA native)
- **tokio** — async runtime for daemon IPC
- **xdotool** — terminal input injection (X11)
- **systemd user service** — daemon lifecycle

## Key Architecture
- Mic (cpal, 16kHz mono) → Silero-VAD (ort) → speech buffer → whisper-rs → LocalAgreement (n=2) → sentence trimmer → xdotool
- Toggle between VOX (always-listening) and PTT (Claude Code native hold-spacebar)

## References
- ufal/whisper_streaming — LocalAgreement policy, academic foundation
- WhisperLiveKit — cleaner wrapper architecture
- whisper_dictation — background daemon pattern

## Status
Implementation in progress.
# currentDate
Today's date is 2026-04-09.

      IMPORTANT: this context may or may not be relevant to your tasks. You should not respond to this context unless it is highly relevant to your task.
