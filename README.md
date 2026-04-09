# whisper-vox

Always-listening voice daemon for Claude Code. Replaces hold-spacebar push-to-talk with continuous listening + sentence debouncing.

## Architecture

```
┌─────────┐    ┌───────────┐    ┌──────────────┐    ┌─────────────┐
│  cpal    │───▶│ Silero-VAD │───▶│ whisper.cpp  │───▶│ LocalAgree  │
│ 16kHz/  │    │  (ort)     │    │ (whisper-rs) │    │   ment-2    │
│ mono/f32│    │ speech/    │    │ small.en     │    │ word-level  │
└─────────┘    │ silence    │    │ int8 CPU     │    │ prefix      │
               └───────────┘    └──────────────┘    └──────┬──────┘
                                                          │
               ┌───────────┐    ┌──────────────┐          │
               │  xdotool  │◀───│  Sentence    │◀─────────┘
               │  type     │    │  Boundary    │
               │  → terminal│    │  Detection   │
               └───────────┘    └──────────────┘
```

**Toggle:** VOX (always-listening) ↔ PTT (Claude Code native hold-spacebar)
**IPC:** Unix socket at `/tmp/whisper-vox.sock`

## Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# System deps
sudo apt install xdotool libclang-dev cmake pkg-config libasound2-dev
```

## Build

```bash
cd whisper-vox
cargo build --release
```

The binary is at `target/release/whisper-vox` (7MB).

Models are downloaded on first run to `~/.cache/whisper-vox/`:
- `silero_vad.onnx` (~2MB)
- `ggml-small.en.bin` (~500MB)

## Usage

```bash
# Start daemon (foreground)
whisper-vox start

# Start with custom config
whisper-vox start --config /path/to/config.yaml

# Toggle VOX ↔ PTT
whisper-vox toggle

# Check status
whisper-vox status

# Stop daemon
whisper-vox stop

# Install as systemd user service
whisper-vox install
```

## Configuration

Default config is bundled. Override at `~/.config/whisper-vox/config.yaml`:

```yaml
audio:
  sample_rate: 16000
  channels: 1
  frame_duration_ms: 30
  device: null          # null = system default

vad:
  threshold: 0.5
  silence_duration_ms: 500   # 300-800ms range
  min_speech_duration_ms: 250

transcriber:
  model_size: "small.en"     # base.en (~150MB) or small.en (~500MB)
  language: "en"

agreement:
  n: 2                       # consecutive passes must agree

injector:
  backend: "xdotool"        # xdotool (X11) or ydotool (Wayland)
  inter_key_delay_ms: 12

daemon:
  mode: "vox"               # vox or ptt
  pid_file: "/tmp/whisper-vox.pid"
  socket_path: "/tmp/whisper-vox.sock"
```

## systemd

```bash
whisper-vox install
systemctl --user enable whisper-vox
systemctl --user start whisper-vox
journalctl --user -u whisper-vox -f
```
