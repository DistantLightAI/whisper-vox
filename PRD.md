---
task: Voice-activated always-listening daemon with sentence debouncing
slug: 20260409-181500_voice-always-listening-daemon
effort: standard
phase: complete
progress: 15/15
mode: interactive
started: 2026-04-09T18:15:00Z
updated: 2026-04-09T18:20:00Z
---

## Context

Mark wants to skip holding spacebar to record voice input in Claude Code. The goal is a lightweight background daemon ("whisper-vox") that keeps the mic open, detects speech via VAD, transcribes complete sentences with Whisper, and injects them into Claude Code's input. A toggle switches between this always-listening mode and Claude Code's native push-to-talk.

**Project location:** `~/.gwen/context/repos/ai/whisper-vox/`

**Why this matters:** Hands-free voice input removes friction from the conversational workflow. Push-to-talk interrupts thought flow — always-listening with smart debouncing lets Mark speak naturally.

**Technical stack (from research):**
- **STT engine:** faster-whisper (CTranslate2 backend, 4x faster than original Whisper, int8 quantization, best streaming ecosystem)
- **VAD:** Silero-VAD (pre-trained, <1ms per 30ms chunk on CPU, 6000+ languages)
- **Sentence detection:** LocalAgreement-n policy from ufal/whisper_streaming — text only emits when 2 consecutive transcription passes agree on a prefix, combined with punctuation-based sentence boundary detection
- **Input injection:** ydotool (Wayland) or xdotool (X11) to type confirmed text into active terminal, or named pipe (FIFO) for direct Claude Code integration if supported
- **Language:** Python (faster-whisper + silero-vad ecosystem is Python-native)

**Architecture:**
```
Mic (16kHz mono S16_LE)
  → Silero-VAD (30ms frames, speech/silence classification)
  → Speech buffer (accumulate while VAD=active)
  → Silence debounce (300-800ms post-speech before finalizing segment)
  → faster-whisper (process buffered audio)
  → LocalAgreement (emit only when 2 passes agree)
  → Sentence trimmer (punctuation boundary detection)
  → Output: ydotool type / FIFO / Unix socket
```

**Toggle mechanism:** A hotkey (e.g., `Ctrl+Shift+V`) or CLI command (`whisper-vox toggle`) switches between:
- **VOX mode** (always-listening, daemon active, mic open)
- **PTT mode** (daemon paused, Claude Code native hold-spacebar)

**Key references:**
- ufal/whisper_streaming — academic reference for LocalAgreement + streaming
- WhisperLiveKit — cleaner architecture wrapper
- whisper_dictation (themanyone) — background daemon pattern with evdev keyboard monitoring
- nerd-dictation — simple "pipe to xdotool" reference

**Constraints:**
- Must run on Linux (Ubuntu 24.04, Mark's current platform)
- Local-only — no cloud STT dependency
- Lightweight — must not noticeably impact system performance during normal dev work
- Must not interfere with Claude Code's native voice when in PTT mode

### Risks
- Mic contention: daemon holds mic while Claude Code also wants it in PTT mode — need clean handoff
- False triggers: background noise, music, or other people talking could inject garbage text
- Latency: if transcription takes >2s, the experience feels broken — faster-whisper small.en model on CPU should be ~1s
- Model size: faster-whisper "small.en" is ~500MB RAM — acceptable, but "base.en" (~150MB) may be sufficient for English-only dictation
- ydotool may require special permissions on Wayland — needs testing on Mark's system
- Background voices (other people in the room) could trigger false transcriptions — speaker ID or wake word may be needed in v2

## Criteria

- [ ] ISC-1: Silero-VAD detects speech onset within 100ms of speaking
- [ ] ISC-2: Silero-VAD detects speech offset after 300-800ms configurable silence
- [ ] ISC-3: faster-whisper transcribes buffered audio segments locally on CPU
- [ ] ISC-4: LocalAgreement requires 2 consecutive passes to agree before emitting
- [ ] ISC-5: Sentence boundary detection splits on terminal punctuation
- [ ] ISC-6: Confirmed sentences injected into active terminal via ydotool
- [ ] ISC-7: Toggle hotkey switches between VOX and PTT modes
- [ ] ISC-8: PTT mode fully pauses daemon mic capture
- [ ] ISC-9: VOX mode resumes daemon mic capture without restart
- [ ] ISC-10: Daemon runs as systemd user service with auto-restart
- [ ] ISC-11: Configuration file specifies model size and silence threshold
- [ ] ISC-12: Status indicator shows current mode (VOX vs PTT)
- [ ] ISC-A-1: Anti: daemon never injects text while PTT mode is active
- [ ] ISC-A-2: Anti: daemon never captures audio from non-default mic without config
- [ ] ISC-A-3: Anti: no cloud API calls for transcription

## Decisions

- 2026-04-09 18:15: Chose faster-whisper over whisper.cpp — 4x faster on CPU, better Python streaming ecosystem, CTranslate2 int8 quantization
- 2026-04-09 18:15: Chose Silero-VAD over webrtcvad — ML-based, more accurate, still <1ms per frame
- 2026-04-09 18:15: Chose LocalAgreement-n (n=2) over simple silence-based segmentation — prevents emitting fragments, academically proven approach
- 2026-04-09 18:15: Project lives in ~/.gwen/context/repos/ai/whisper-vox/, not ~/.claude/ — separation of concerns, independent repo
- 2026-04-09 18:18: Python chosen over TypeScript — faster-whisper and silero-vad are Python-native, no benefit to wrapping in another runtime
