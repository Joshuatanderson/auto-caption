# AutoCap

A macOS desktop app that takes a video file and produces a captioned version with styled word-level subtitles burned in. Local-first: no cloud uploads, no API keys, no subscriptions.

Built as an alternative to Riverside / Submagic / etc. for longer-form LinkedIn-style content — 2–30 minute videos, phrase-level captions with a single moving highlight, hackable styling.

## Download

**[Download the latest release (macOS, Apple Silicon)](https://github.com/Joshuatanderson/auto-caption/releases/latest/download/AutoCap-macos-arm64.dmg)**

Or browse all releases: [github.com/Joshuatanderson/auto-caption/releases](https://github.com/Joshuatanderson/auto-caption/releases)

Open the `.dmg`, drag **AutoCap.app** into `/Applications`, then see [Prerequisites](#prerequisites) for the one-time system dependency install.

### About the scary "can't be opened" warning

The DMG is **not signed or notarized** — I don't pay Apple the $99/year Developer Program fee to code-sign a free side project. So the first time you launch AutoCap, macOS Gatekeeper will show something like:

> **"AutoCap" can't be opened because Apple cannot check it for malicious software.**
>
> …or: *"AutoCap" is damaged and can't be opened.*

This doesn't mean anything is actually wrong with the app — it just means Apple hasn't been paid to vouch for it. If you don't trust me, read the source: everything that runs is in this repo and you can [build it yourself](#building-from-source).

**To open it anyway:**

- **Preferred:** In Finder, right-click (or Ctrl-click) `AutoCap.app` and choose **Open**, then click **Open** on the prompt. Normal double-click works forever after.
- **If right-click doesn't show an Open option,** remove the quarantine flag from a terminal:

  ```bash
  xattr -dr com.apple.quarantine /Applications/AutoCap.app
  ```

## What it does

Drop an `.mp4` in, pick one or more output formats (Unchanged, YT Shorts 9:16, LI Shorts 4:5, Square 1:1), click Generate. The app:

1. Extracts mono 16 kHz audio with `ffmpeg`.
2. Transcribes with `whisper-cli` (whisper.cpp) using DTW-aligned per-token timestamps.
3. Merges BPE sub-word tokens into whole words, groups them into phrases, and emits an ASS subtitle file with a two-layer rendering (sharp text on top, accent-colored halo behind).
4. Burns the captions into the video via `ffmpeg` + libass, using VideoToolbox hardware h.264 encoding.

Each run produces a self-contained `<stem>_export_<unix_secs>/` folder with the final `.mp4` at the top and WAV / JSON / ASS artifacts under `artifacts/`.

## Prerequisites

AutoCap shells out to two installed CLIs and reads one model file from disk. It does not bundle any of them — install them yourself:

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

Set `AUTOCAP_WHISPER_MODEL` to the absolute path of a `.bin` ggml whisper model. The pipeline uses DTW alignment tied to the `large.v3.turbo` head layout, so pointing this at a different architecture (base, small, large-v2, …) will produce misaligned per-word timestamps. Keep it on a large-v3-turbo build.

```bash
export AUTOCAP_WHISPER_MODEL=/Volumes/Scratch/models/large-v3-turbo.bin
```

When launching the `.app` from Finder, env vars aren't inherited — either `open -a AutoCap.app` from a shell that has the var set, or use the default path.

### Dependency auto-check on first launch

On first launch AutoCap probes `$PATH` plus common install locations (`/opt/homebrew/bin`, `/usr/local/bin`, `/opt/local/bin`, `~/.local/bin`, `~/bin`) for each binary, and similarly for the model file. Hits are cached in SQLite so subsequent launches skip the search.

If anything's missing, the app shows a red panel at the top with a copy-pasteable install prompt designed for Claude Code / any AI coding assistant — hit **Copy prompt**, paste it into your assistant, let it install the deps, then click **Re-check**.

## Running from source

```bash
bun install
bun run tauri dev
```

Use the Tauri window that opens — **not** `bun run dev`, which spins up the frontend in a browser tab. The browser tab has a no-op drag-drop handler and no native file picker, so you won't be able to load anything through it.

## Building from source

```bash
bun run tauri build
```

Produces under `src-tauri/target/release/bundle/`:

- `macos/AutoCap.app` — the double-clickable app bundle
- `dmg/AutoCap_<version>_aarch64.dmg` — a DMG installer

The build is **not signed or notarized** — see the [warning above](#about-the-scary-cant-be-opened-warning). Users still need `whisper-cli` + `ffmpeg` + the model on their own machine; the app is a thin orchestrator, not a self-contained ML bundle.

To ship signed/notarized binaries, you'd need an Apple Developer ID certificate and `codesign` + `xcrun notarytool` wiring in CI.

## Repo layout

- `src-tauri/` — Rust backend: pipeline orchestration, DB, Tauri commands.
  - `src/pipeline/` — one module per stage (audio, transcribe, ass, burn, probe, types). All shell-outs live here.
  - `src/db.rs` — SQLite schema, theme / settings rows, Tauri commands for reads/writes.
  - `src/commands.rs` — the single `run_pipeline` command the frontend invokes.
- `src/` — Svelte 5 frontend (single screen, no routing).
- `static/fonts/` — Noto Sans TTFs bundled as Tauri resources so libass can find them regardless of the user's font installation.

## License

- AutoCap itself is licensed under the [Apache License 2.0](./LICENSE).
- The bundled Noto Sans fonts under `static/fonts/` are licensed separately under the SIL Open Font License 1.1 — see [`static/fonts/OFL.md`](./static/fonts/OFL.md).

## Design principles

See `CLAUDE.md` for the longer version. Short version: thin Rust orchestration, all styling decisions live in ASS generation, each pipeline stage produces an inspectable artifact, and the pipeline is a pure function from `(input_video, style) → output_video` with no hidden state.
