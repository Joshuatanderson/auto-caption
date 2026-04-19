# Captioner

A macOS desktop app that takes a video file and produces a captioned version with styled word-level subtitles burned in. Local-first: no cloud uploads, no API keys, no subscriptions.

Built as an alternative to Riverside / Submagic / etc. for longer-form LinkedIn-style content — 2–30 minute videos, phrase-level captions with a single moving highlight, hackable styling.

## What it does

Drop an `.mp4` in, pick one or more output formats (Unchanged, YT Shorts 9:16, LI Shorts 4:5, Square 1:1), click Generate. The app:

1. Extracts mono 16 kHz audio with `ffmpeg`.
2. Transcribes with `whisper-cli` (whisper.cpp) using DTW-aligned per-token timestamps.
3. Merges BPE sub-word tokens into whole words, groups them into phrases, and emits an ASS subtitle file with a two-layer rendering (sharp text on top, accent-colored halo behind).
4. Burns the captions into the video via `ffmpeg` + libass, using VideoToolbox hardware h.264 encoding.

Each run produces a self-contained `<stem>_export_<unix_secs>/` folder with the final `.mp4` at the top and WAV / JSON / ASS artifacts under `artifacts/`.

## Prerequisites

Captioner shells out to two installed CLIs and reads one model file from disk. It does not bundle any of them — install them yourself:

| Dependency | Install | Why |
|---|---|---|
| **macOS 26+ on Apple Silicon (M-series)** | — | VideoToolbox encoder + WKWebView |
| **`whisper-cli`** (whisper.cpp) | `brew install whisper-cpp` | Transcription |
| **`ffmpeg`** (with libass + fontconfig) | `brew install ffmpeg` | Audio extract + caption burn-in |
| **whisper large-v3-turbo model** | See below | The actual weights |

The app expects the three CLIs on `$PATH` (Homebrew's default install location handles this) and the model at `~/.local/models/whisper/large-v3-turbo.bin` unless overridden.

### Fetching the whisper model

```bash
mkdir -p ~/.local/models/whisper
# Roughly 1.5 GB download from Hugging Face:
curl -L -o ~/.local/models/whisper/large-v3-turbo.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin
```

### Overriding the model path

Set `CAPTIONER_WHISPER_MODEL` to the absolute path of a `.bin` ggml whisper model. The pipeline uses DTW alignment tied to the `large.v3.turbo` head layout, so pointing this at a different architecture (base, small, large-v2, …) will produce misaligned per-word timestamps. Keep it on a large-v3-turbo build.

```bash
export CAPTIONER_WHISPER_MODEL=/Volumes/Scratch/models/large-v3-turbo.bin
```

When launching the `.app` from Finder, env vars aren't inherited — either `open -a vid-pipeline.app` from a shell that has the var set, or use the default path.

## Running from source

```bash
bun install
bun run tauri dev
```

Use the Tauri window that opens — **not** `bun run dev`, which spins up the frontend in a browser tab. The browser tab has a no-op drag-drop handler and no native file picker, so you won't be able to load anything through it.

## Building a distributable

```bash
bun run tauri build
```

Produces under `src-tauri/target/release/bundle/`:

- `macos/vid-pipeline.app` — the double-clickable app bundle
- `dmg/vid-pipeline_<version>_aarch64.dmg` — a DMG installer

The build is **not signed or notarized**. When someone else downloads it, macOS Gatekeeper will refuse to open it until they right-click → Open, or run `xattr -dr com.apple.quarantine /Applications/vid-pipeline.app` once after installing. They still need whisper-cli + ffmpeg + the model on their own machine — this app is a thin orchestrator, not a self-contained ML bundle.

To ship signed/notarized binaries, you'd need an Apple Developer ID certificate and `codesign` + `xcrun notarytool` wiring in CI.

## Repo layout

- `src-tauri/` — Rust backend: pipeline orchestration, DB, Tauri commands.
  - `src/pipeline/` — one module per stage (audio, transcribe, ass, burn, probe, types). All shell-outs live here.
  - `src/db.rs` — SQLite schema, theme / settings rows, Tauri commands for reads/writes.
  - `src/commands.rs` — the single `run_pipeline` command the frontend invokes.
- `src/` — Svelte 5 frontend (single screen, no routing).
- `static/fonts/` — Noto Sans TTFs bundled as Tauri resources so libass can find them regardless of the user's font installation.

## Design principles

See `CLAUDE.md` for the longer version. Short version: thin Rust orchestration, all styling decisions live in ASS generation, each pipeline stage produces an inspectable artifact, and the pipeline is a pure function from `(input_video, style) → output_video` with no hidden state.
