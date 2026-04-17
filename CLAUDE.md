# Captioner

A macOS desktop app that takes a video file and produces a captioned version with styled word-level subtitles burned in.

## Why this exists

Paid caption tools (Riverside, Submagic, etc.) have quality issues: captions stuck in one screen position, inflexible styling, cloud upload requirements, subscription pricing. This is a local-first, open-source alternative optimized for longer-form LinkedIn content.

## Goals

- **Drop a video, get a captioned video back.** Minimum friction.
- **Local-only processing.** No uploads, no API keys, no network dependencies at runtime.
- **Word-level timing accuracy.** Captions should highlight the specific word being spoken, not just the phrase.
- **Hackable styling.** Style parameters should be easy to tweak without touching rendering code.
- **Longer-form optimized.** Target 2-30 minute videos, not TikTok clips. Phrase-level captions with one accent word, not full karaoke.

## Non-goals (for v1)

- Multi-speaker diarization
- Real-time / streaming transcription
- Translation or dubbing
- Cloud sync or collaboration features
- Windows or Linux support
- Social media upload integration

## Stack

- **Desktop shell**: Tauri 2 (Rust backend + webview frontend)
- **UI**: Svelte 5 + TypeScript + Vite
- **Transcription**: whisper.cpp via Homebrew CLI (`whisper-cli`), model: large-v3-turbo
- **Audio/video processing**: ffmpeg via CLI
- **Subtitle format**: Advanced SubStation Alpha (.ass) for styled burn-in
- **Target platform**: macOS on Apple Silicon (M-series)

## Architecture philosophy

- **Thin Rust orchestration layer.** Rust's job is to shell out to `whisper-cli` and `ffmpeg`, parse their output, and coordinate the pipeline. It is not doing ML itself.
- **All styling logic lives in ASS generation.** The `.ass` subtitle file is the source of truth for how captions look. Changing styles = changing ASS generation, nothing else.
- **Every stage produces inspectable artifacts.** `audio.wav`, `transcript.json`, `subtitles.ass`, `output.mp4` should all be persistable and debuggable individually.
- **No hidden state.** The pipeline is a pure function: `(input_video, style) -> output_video`. No database, no global state.

## Current state

See ROADMAP.md for what's built vs what's next.

## Hard dependencies on the host machine

- macOS 26+ (Tahoe)
- Apple Silicon
- `whisper-cli` installed via Homebrew (`brew install whisper-cpp`)
- `ffmpeg` with libass + fontconfig (`brew install ffmpeg`)
- Whisper model at `~/.local/models/whisper/large-v3-turbo.bin` (symlinked to HF cache)
- Rust 1.70+ and Node 20+ for development

## Repo conventions

- **`src-tauri/`** — Rust backend. Commands exposed to the frontend go in `lib.rs`.
- **`src/`** — Svelte frontend. One screen, no routing.
- **Test artifacts** — sample videos and expected outputs in `test-artifacts/`, gitignored.
- **Transcripts** — ad-hoc transcripts produced during dev live in `transcripts/`, gitignored.

## Opinionated defaults

- **Phrase-level captions, not word-by-word.** 4-6 words per phrase, one word highlighted in accent color. Research shows this beats TikTok-style single-word flash for content over 90 seconds.
- **Burn captions in by default.** LinkedIn autoplays muted and its native caption rendering is poor. We always produce a captioned MP4, not a sidecar SRT.
- **Fail loud.** If any pipeline stage fails, surface the full stderr from the underlying tool to the user. No "something went wrong" errors.

## What to ask when in doubt

- Is this adding complexity to the MVP, or is it genuinely required for the next validation step? Default to deferring.
- Does this hide state that should be visible? If yes, don't add it.
- Could this be solved by a style parameter change instead of a code change? Prefer the parameter.

<!-- hippo:start -->
## Project Memory (Hippo)

Before starting work, load relevant context:
```bash
hippo context --auto --budget 1500
```

When you learn something important:
```bash
hippo remember "<lesson>"
```

When you hit an error or discover a gotcha:
```bash
hippo remember "<what went wrong and why>" --error
```

After significant discussions or decisions, capture context:
```bash
hippo capture --stdin <<< 'summary of what was decided'
```

After completing work successfully:
```bash
hippo outcome --good
```
<!-- hippo:end -->
