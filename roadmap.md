# Captioner Roadmap

## Operating principle

**Validate every phase before moving to the next.** Each phase ends with a concrete, verifiable output. Do not stack phases without checkpointing. If a phase doesn't validate, diagnose before proceeding — a broken foundation compounds.

Phases are sequential. Stretch goals are clearly marked and should not be pulled forward unless the MVP is complete.

---

## Phase 0: Toolchain (Done)

Baseline dependencies installed and validated on the host machine.

- [x] whisper.cpp installed, `whisper-cli` on PATH
- [x] Large-v3-turbo model downloaded and symlinked to stable path
- [x] ffmpeg installed with libass + fontconfig
- [x] Rust + Node toolchains ready
- [x] Tauri 2 + Svelte scaffold generated (demo runs)

**Validation gate:** `whisper-cli`, `ffmpeg`, and `npm run tauri dev` all work from the terminal.

---

## Phase 1: Gut the demo and validate drag-and-drop

Strip the Tauri scaffold down to a blank screen that accepts a dropped video file and displays its absolute path. Nothing else.

**Deliverables:**
- Blank Svelte UI (delete greet demo).
- Drop zone that accepts MP4 files.
- On drop, the file's absolute path is visible in the UI.
- Non-MP4 files are rejected with a clear message.

**Validation gate:** Drop a `.mp4` onto the app window. The path appears. Drop a `.txt`. It's rejected with a message.

---

## Phase 2: Extract button → transcript

Add a button that runs the transcription pipeline on the dropped file and displays the raw JSON transcript in the UI. No styling, no rendering, just proof that the pipeline works.

**Deliverables:**
- "Extract" button that becomes enabled once a file is dropped.
- Rust command that:
  1. Runs `ffmpeg` to extract a 16kHz mono WAV from the dropped MP4.
  2. Runs `whisper-cli` against the WAV with `--output-json --output-words`.
  3. Reads the resulting JSON file.
  4. Returns the parsed JSON to the frontend.
- UI displays a loading state while the command runs.
- UI shows the raw JSON transcript (or pretty-printed) on success.
- UI shows an error message on failure.

**Validation gate:** Drop a 30-second English video, hit Extract, see a transcript with word-level timestamps in the UI within a minute. Drop a different video, repeat, get a different transcript.

---

## Phase 3: Generate .ass subtitle file

Take the word-level JSON from Phase 2 and generate a well-formed `.ass` subtitle file. Save it to disk. Do not render video yet.

**Deliverables:**
- Rust module that converts the whisper JSON structure to an ASS file.
- Hardcoded style values (default font, size, colors, position). No style parameters yet.
- "Generate captions" button that produces `subtitles.ass` next to the input file.
- UI shows a "file saved" confirmation with the output path.

**Validation gate:** Open the generated `.ass` file in a text editor — the structure is readable, timestamps look correct. Play the original MP4 in VLC and manually load the `.ass` file — captions appear on top of the video at the right moments.

---

## Phase 4: Burn captions into video

Run ffmpeg with the ASS filter to produce a captioned MP4 output. This closes the loop on the MVP.

**Deliverables:**
- Rust command that invokes ffmpeg with `-vf "ass=subtitles.ass"` and VideoToolbox hardware encoding.
- Output MP4 saved next to input with a `_captioned` suffix.
- "Burn captions" button, enabled once ASS is generated.
- UI shows progress (even crude — "encoding..." is fine) and a "Show in Finder" button on completion.

**Validation gate:** Drop a video → Extract → Generate → Burn. Open the resulting `_captioned.mp4` in QuickTime. Captions are visibly burned in, timed to speech, readable on the video background.

**This is the MVP.** Everything above is the minimum for a working captioner.

---

## Stretch goals (do not start until MVP validates end-to-end)

### Stretch 1: Single "Do everything" button

Collapse Extract → Generate → Burn into one action triggered by a single button or auto-triggered on drop. Show per-stage progress.

**Validation gate:** Drop a file, click one button, get a captioned video out. No manual stage-stepping.

---

### Stretch 2: Style parameters (predefined presets)

Add a dropdown of 3-5 named style presets (e.g., "Default", "Bold Red", "Subtle", "Brand"). Passing a preset name to the Rust command changes the ASS generation.

**Deliverables:**
- Preset definitions (font, size, primary color, highlight color, outline, position) as typed structs.
- Dropdown in UI.
- Selected preset flows through to ASS generation.

**Validation gate:** Run the same video through three different presets. All three output files have visibly different caption styling.

---

### Stretch 3: Arbitrary style parameters

Expose individual style controls in the UI (font size slider, color pickers, position toggle). User can compose their own style without editing code.

**Deliverables:**
- UI controls for: font size, primary color, highlight color, outline width, vertical position, words per phrase.
- Live preview of one caption line with current style (nice-to-have).
- Save/load custom presets (stretch within stretch).

**Validation gate:** User changes font size from 60 to 100 via slider, re-runs burn, output video shows larger captions.

---

## Stretch goals worth noting but not scheduled

- Silence trimming as a pre-processing step (Auto-Editor integration)
- Face-detection-driven caption position flipping (captions move to top when a face is in the lower third)
- Brand logo burn-in alongside captions
- Output in multiple aspect ratios (9:16, 1:1, 16:9) in one pass
- Preview of first few seconds before committing to full encode
- SRT sidecar file output for accessibility uploads

These are explicitly out of scope until stretches 1-3 are done. Do not pull them forward.

---

## Anti-goals for this project

If the scope expands to include these, stop and reconsider whether a different tool is the right answer:

- Multi-speaker diarization (requires different models)
- Non-English languages as primary use case (consider Parakeet or WhisperX)
- Live/streaming transcription
- Cloud processing or team collaboration

---

## How to use this document

- Before starting a phase, reread its validation gate. That's what "done" means.
- When a phase validates, commit the code and mark the checkbox.
- When a phase fails validation, stop, diagnose, and fix before moving on. Do not proceed with known broken foundations.
- If a stretch goal starts creeping into MVP work, stop and ask: "does this block validation of the current phase?" If not, defer it.
