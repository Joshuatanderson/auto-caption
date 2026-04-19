use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pipeline::types::StageError;

/// Where to write the extracted WAV. When `dest_dir` is `Some`, the wav lands
/// at `<dest_dir>/<stem>.wav` (used for routing into an export folder's
/// artifacts subdir). When `None`, keeps the legacy "next to input" behavior.
pub fn audio_output_path(input: &Path, dest_dir: Option<&Path>) -> PathBuf {
    match dest_dir {
        Some(dir) => {
            let stem = input.file_stem().unwrap_or_default();
            let mut p = dir.join(stem);
            p.set_extension("wav");
            p
        }
        None => input.with_extension("wav"),
    }
}

/// Builds the ffmpeg argument list for 16kHz mono WAV extraction. Pure function.
pub fn build_extract_audio_args(input: &Path, output: &Path) -> Vec<String> {
    vec![
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().into_owned(),
        "-vn".to_string(),
        "-ar".to_string(),
        "16000".to_string(),
        "-ac".to_string(),
        "1".to_string(),
        output.to_string_lossy().into_owned(),
    ]
}

pub fn run_extract_audio(input: &Path, dest_dir: Option<&Path>) -> Result<PathBuf, StageError> {
    let output = audio_output_path(input, dest_dir);
    let args = build_extract_audio_args(input, &output);

    let result = Command::new("ffmpeg").args(&args).output().map_err(|e| StageError {
        stage: "extract_audio".to_string(),
        message: format!("Failed to spawn ffmpeg: {e}"),
        stderr: None,
    })?;

    if !result.status.success() {
        return Err(StageError {
            stage: "extract_audio".to_string(),
            message: "ffmpeg exited with non-zero status".to_string(),
            stderr: Some(String::from_utf8_lossy(&result.stderr).into_owned()),
        });
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn audio_output_path_replaces_extension() {
        let input = Path::new("/tmp/video.mp4");
        assert_eq!(audio_output_path(input, None), Path::new("/tmp/video.wav"));
    }

    #[test]
    fn audio_output_path_no_extension() {
        let input = Path::new("/tmp/video");
        assert_eq!(audio_output_path(input, None), Path::new("/tmp/video.wav"));
    }

    #[test]
    fn audio_output_path_with_dest_dir() {
        let input = Path::new("/tmp/video.mp4");
        let dest = Path::new("/exports/abc/artifacts");
        assert_eq!(
            audio_output_path(input, Some(dest)),
            Path::new("/exports/abc/artifacts/video.wav"),
        );
    }

    #[test]
    fn audio_output_path_with_dest_dir_ignores_input_parent() {
        // Even if the input is deep in the source tree, the dest dir wins.
        let input = Path::new("/home/user/Movies/2025/clip.mp4");
        let dest = Path::new("/outputs/run1/artifacts");
        assert_eq!(
            audio_output_path(input, Some(dest)),
            Path::new("/outputs/run1/artifacts/clip.wav"),
        );
    }

    #[test]
    fn build_extract_audio_args_correct() {
        let input = Path::new("/tmp/video.mp4");
        let output = Path::new("/tmp/video.wav");
        let args = build_extract_audio_args(input, output);
        assert_eq!(args[0], "-y");
        assert_eq!(args[1], "-i");
        assert_eq!(args[2], "/tmp/video.mp4");
        assert!(args.contains(&"-vn".to_string()));
        assert!(args.contains(&"16000".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert_eq!(args.last().unwrap(), "/tmp/video.wav");
    }

    #[test]
    #[ignore = "requires ffmpeg on PATH and a real test-artifacts/sample.mp4"]
    fn run_extract_audio_on_sample() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let input = manifest.parent().unwrap().join("test-artifacts/sample.mp4");
        assert!(input.exists(), "test-artifacts/sample.mp4 not found");
        let result = run_extract_audio(&input, None);
        assert!(result.is_ok(), "extract_audio failed: {:?}", result.err());
    }
}
