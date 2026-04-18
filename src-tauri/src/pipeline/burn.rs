use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pipeline::types::StageError;

pub fn burn_output_path(input: &Path) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let parent = input.parent().unwrap_or(Path::new("."));
    parent.join(format!("{stem}_captioned.mp4"))
}

/// Builds ffmpeg burn-in arguments using VideoToolbox hardware encoder. Pure function.
pub fn build_burn_args(input: &Path, ass_path: &Path, output: &Path) -> Vec<String> {
    vec![
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().into_owned(),
        "-vf".to_string(),
        format!("ass={}", ass_path.to_string_lossy()),
        "-c:v".to_string(),
        "h264_videotoolbox".to_string(),
        "-c:a".to_string(),
        "copy".to_string(),
        output.to_string_lossy().into_owned(),
    ]
}

pub fn run_burn(input: &Path, ass_path: &Path) -> Result<PathBuf, StageError> {
    let output = burn_output_path(input);
    let args = build_burn_args(input, ass_path, &output);

    let result = Command::new("ffmpeg").args(&args).output().map_err(|e| StageError {
        stage: "burn_captions".to_string(),
        message: format!("Failed to spawn ffmpeg: {e}"),
        stderr: None,
    })?;

    if !result.status.success() {
        return Err(StageError {
            stage: "burn_captions".to_string(),
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
    fn burn_output_path_appends_captioned() {
        let input = Path::new("/videos/myvideo.mp4");
        assert_eq!(burn_output_path(input), Path::new("/videos/myvideo_captioned.mp4"));
    }

    #[test]
    fn burn_output_path_same_directory() {
        let input = Path::new("/tmp/test.mp4");
        let out = burn_output_path(input);
        assert_eq!(out.parent().unwrap(), Path::new("/tmp"));
    }

    #[test]
    fn build_burn_args_contains_videotoolbox() {
        let input = Path::new("/tmp/video.mp4");
        let ass = Path::new("/tmp/video.ass");
        let output = Path::new("/tmp/video_captioned.mp4");
        let args = build_burn_args(input, ass, output);
        assert!(args.contains(&"h264_videotoolbox".to_string()));
        assert!(args.iter().any(|a| a.starts_with("ass=")));
        assert_eq!(args.last().unwrap(), "/tmp/video_captioned.mp4");
    }

    #[test]
    fn build_burn_args_audio_copy() {
        let input = Path::new("/tmp/video.mp4");
        let ass = Path::new("/tmp/video.ass");
        let output = Path::new("/tmp/video_captioned.mp4");
        let args = build_burn_args(input, ass, output);
        let copy_pos = args.iter().position(|a| a == "copy").unwrap();
        assert_eq!(args[copy_pos - 1], "-c:a");
    }

    #[test]
    #[ignore = "requires ffmpeg on PATH and real test-artifacts/sample.mp4 + sample.ass"]
    fn run_burn_on_sample() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.parent().unwrap();
        let input = root.join("test-artifacts/sample.mp4");
        let ass = root.join("test-artifacts/sample.ass");
        assert!(input.exists() && ass.exists());
        let result = run_burn(&input, &ass);
        assert!(result.is_ok(), "burn failed: {:?}", result.err());
    }
}
