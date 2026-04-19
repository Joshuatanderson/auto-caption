use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pipeline::types::{StageError, WhisperOutput};

/// whisper-cli with `-of <stem>` writes `<stem>.json`.
/// stem = wav path with .wav extension removed.
pub fn whisper_json_output_path(wav_path: &Path) -> PathBuf {
    wav_path.with_extension("json")
}

/// Builds whisper-cli argument list.
///
/// Flags used:
///   -ojf  = output JSON (full, includes per-token timing + probabilities)
///   -of   = output file path WITHOUT extension (whisper appends .json)
///   --dtw = post-pass token-level alignment via DTW on cross-attention
///           heads; tightens per-token timestamps from ~100–300 ms typical
///           down to ~50–100 ms. The preset `large.v3.turbo` matches the
///           alignment-head layout of the large-v3-turbo model we load.
///
/// `--output-dir` does NOT exist in whisper-cli; `-of` is the correct way to
/// control where the output file lands.
///
/// `--output-words` / `-owts` generates a karaoke subtitle file, not JSON
/// tokens — intentionally omitted here.
pub fn build_whisper_args(wav_path: &Path, model_path: &Path, output_stem: &Path) -> Vec<String> {
    vec![
        "--model".to_string(),
        model_path.to_string_lossy().into_owned(),
        "-ojf".to_string(),  // full JSON: includes tokens with timestamps + probabilities
        "--dtw".to_string(),
        "large.v3.turbo".to_string(),
        "-of".to_string(),
        output_stem.to_string_lossy().into_owned(),
        wav_path.to_string_lossy().into_owned(),
    ]
}

/// Resolves the whisper model path. Honors `AUTOCAP_WHISPER_MODEL` if set
/// (lets users keep the model outside `~/.local` without a rebuild); otherwise
/// falls back to the documented default. The DTW preset downstream is still
/// tied to large-v3-turbo, so overriding to a different architecture will
/// misalign timestamps — document this constraint for operators.
pub fn default_model_path() -> PathBuf {
    if let Ok(p) = std::env::var("AUTOCAP_WHISPER_MODEL") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/models/whisper/large-v3-turbo.bin")
}

/// Parses whisper-cli JSON output. Pure function — no I/O.
pub fn parse_whisper_json(json_str: &str) -> Result<WhisperOutput, StageError> {
    serde_json::from_str(json_str).map_err(|e| StageError {
        stage: "transcribe".to_string(),
        message: format!("Failed to parse whisper JSON: {e}"),
        stderr: None,
    })
}

pub fn run_transcribe(wav_path: &Path) -> Result<WhisperOutput, StageError> {
    let output_stem = wav_path.with_extension("");
    let model = default_model_path();
    let args = build_whisper_args(wav_path, &model, &output_stem);

    let result = Command::new("whisper-cli").args(&args).output().map_err(|e| StageError {
        stage: "transcribe".to_string(),
        message: format!("Failed to spawn whisper-cli: {e}"),
        stderr: None,
    })?;

    if !result.status.success() {
        return Err(StageError {
            stage: "transcribe".to_string(),
            message: "whisper-cli exited with non-zero status".to_string(),
            stderr: Some(String::from_utf8_lossy(&result.stderr).into_owned()),
        });
    }

    let json_path = whisper_json_output_path(wav_path);
    let json_str = std::fs::read_to_string(&json_path).map_err(|e| StageError {
        stage: "transcribe".to_string(),
        message: format!("Could not read whisper JSON at {}: {e}", json_path.display()),
        stderr: None,
    })?;

    parse_whisper_json(&json_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn whisper_json_output_path_replaces_wav_extension() {
        let wav = Path::new("/tmp/audio.wav");
        assert_eq!(whisper_json_output_path(wav), Path::new("/tmp/audio.json"));
    }

    #[test]
    fn whisper_json_output_path_preserves_dir() {
        let wav = Path::new("/home/user/videos/clip.wav");
        let out = whisper_json_output_path(wav);
        assert_eq!(out.parent().unwrap(), Path::new("/home/user/videos"));
        assert_eq!(out.file_name().unwrap(), "clip.json");
    }

    #[test]
    fn build_whisper_args_uses_of_flag() {
        let wav = Path::new("/tmp/audio.wav");
        let model = Path::new("/models/whisper.bin");
        let stem = Path::new("/tmp/audio");
        let args = build_whisper_args(wav, model, stem);
        assert!(args.contains(&"-ojf".to_string()));
        assert!(args.contains(&"-of".to_string()));
        assert!(args.contains(&"--model".to_string()));
        // DTW preset must be wired up for tight per-token timestamps
        assert!(args.contains(&"--dtw".to_string()));
        let dtw_pos = args.iter().position(|a| a == "--dtw").unwrap();
        assert_eq!(args[dtw_pos + 1], "large.v3.turbo");
        // must NOT use the old broken flag
        assert!(!args.contains(&"--output-dir".to_string()));
        assert!(!args.contains(&"--output-words".to_string()));
        assert_eq!(args.last().unwrap(), "/tmp/audio.wav");
    }

    #[test]
    fn build_whisper_args_of_path_is_stem() {
        let wav = Path::new("/tmp/audio.wav");
        let model = Path::new("/models/large.bin");
        let stem = Path::new("/tmp/audio");
        let args = build_whisper_args(wav, model, stem);
        let of_pos = args.iter().position(|a| a == "-of").unwrap();
        assert_eq!(args[of_pos + 1], "/tmp/audio");
    }

    #[test]
    fn build_whisper_args_model_path_follows_flag() {
        let wav = Path::new("/tmp/audio.wav");
        let model = Path::new("/models/large.bin");
        let stem = Path::new("/tmp/audio");
        let args = build_whisper_args(wav, model, stem);
        let model_pos = args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(args[model_pos + 1], "/models/large.bin");
    }

    #[test]
    fn parse_whisper_json_minimal() {
        let json = r#"{"transcription":[{"timestamps":{"from":"00:00:00,000","to":"00:00:01,000"},"offsets":{"from":0,"to":1000},"text":" hello","tokens":[{"text":" hello","timestamps":{"from":"00:00:00,000","to":"00:00:01,000"},"offsets":{"from":0,"to":1000},"id":1234,"p":0.95}]}]}"#;
        let result = parse_whisper_json(json);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let output = result.unwrap();
        assert_eq!(output.transcription.len(), 1);
        assert_eq!(output.transcription[0].tokens.len(), 1);
        assert!((output.transcription[0].tokens[0].p - 0.95).abs() < 1e-6);
    }

    #[test]
    fn parse_whisper_json_invalid_returns_error() {
        let result = parse_whisper_json("not json at all");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().stage, "transcribe");
    }

    #[test]
    #[ignore = "requires whisper-cli on PATH and real test-artifacts/sample.wav"]
    fn run_transcribe_on_sample() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let wav = manifest.parent().unwrap().join("test-artifacts/sample.wav");
        assert!(wav.exists(), "test-artifacts/sample.wav not found");
        let result = run_transcribe(&wav);
        assert!(result.is_ok(), "transcribe failed: {:?}", result.err());
        assert!(!result.unwrap().transcription.is_empty());
    }
}
