use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::pipeline::types::StageError;

#[derive(Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    width: u32,
    height: u32,
}

/// Reads the first video stream's pixel dimensions via ffprobe.
pub fn probe_dimensions(ffprobe_path: &Path, path: &Path) -> Result<(u32, u32), StageError> {
    let result = Command::new(ffprobe_path)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| StageError {
            stage: "probe".to_string(),
            message: format!("Failed to spawn ffprobe: {e}"),
            stderr: None,
        })?;

    if !result.status.success() {
        return Err(StageError {
            stage: "probe".to_string(),
            message: "ffprobe exited with non-zero status".to_string(),
            stderr: Some(String::from_utf8_lossy(&result.stderr).into_owned()),
        });
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&result.stdout).map_err(|e| StageError {
        stage: "probe".to_string(),
        message: format!("Failed to parse ffprobe JSON: {e}"),
        stderr: Some(String::from_utf8_lossy(&result.stdout).into_owned()),
    })?;

    let stream = parsed.streams.first().ok_or_else(|| StageError {
        stage: "probe".to_string(),
        message: "No video stream found in input".to_string(),
        stderr: None,
    })?;

    Ok((stream.width, stream.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires ffprobe on PATH and a real test-artifacts/sample.mp4"]
    fn probe_dimensions_on_sample() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let input = manifest.parent().unwrap().join("test-artifacts/sample.mp4");
        assert!(input.exists(), "test-artifacts/sample.mp4 not found");
        let dims = probe_dimensions(Path::new("ffprobe"), &input).expect("probe failed");
        assert!(dims.0 > 0 && dims.1 > 0);
    }
}
