use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pipeline::types::{FormatSpec, StageError};

pub fn burn_output_path(folder: &Path, stem: &str, slug: &str) -> PathBuf {
    folder.join(format!("{stem}_{slug}.mp4"))
}

/// Largest centered rect with target aspect that fits inside (iw, ih).
/// Returned dims are rounded down to even pixels (ffmpeg+h264 want even).
pub fn center_crop_dims(iw: u32, ih: u32, tw: u32, th: u32) -> (u32, u32) {
    // iw/ih vs tw/th, compared via cross-multiplication to avoid floats.
    // input_wider_or_eq: iw * th >= ih * tw  → crop width, keep height
    // input_narrower:    iw * th <  ih * tw  → crop height, keep width
    let input_wider_or_eq = (iw as u64) * (th as u64) >= (ih as u64) * (tw as u64);
    let (cw, ch) = if input_wider_or_eq {
        let crop_w = ((ih as u64) * (tw as u64) / (th as u64)) as u32;
        (crop_w, ih)
    } else {
        let crop_h = ((iw as u64) * (th as u64) / (tw as u64)) as u32;
        (iw, crop_h)
    };
    (cw & !1, ch & !1)
}

/// Builds the `-vf` filter chain: crop + scale + ass burn.
/// When output dims match input, skips crop/scale and only burns the ASS.
/// `fonts_dir` is passed as `fontsdir=` to libass so it can find bundled fonts.
pub fn build_vf_chain(
    ass_path: &Path,
    format: &FormatSpec,
    input_w: u32,
    input_h: u32,
    fonts_dir: Option<&Path>,
) -> String {
    let ass = ass_path.to_string_lossy();
    let ass_filter = match fonts_dir {
        Some(dir) => format!("ass={ass}:fontsdir={dir}", dir = dir.to_string_lossy()),
        None => format!("ass={ass}"),
    };
    if format.width == input_w && format.height == input_h {
        return ass_filter;
    }
    let (cw, ch) = center_crop_dims(input_w, input_h, format.width, format.height);
    format!(
        "crop={cw}:{ch}:(iw-{cw})/2:(ih-{ch})/2,scale={tw}:{th},{ass_filter}",
        cw = cw,
        ch = ch,
        tw = format.width,
        th = format.height,
        ass_filter = ass_filter,
    )
}

/// Builds ffmpeg burn-in arguments using VideoToolbox hardware encoder. Pure function.
pub fn build_burn_args(
    input: &Path,
    ass_path: &Path,
    output: &Path,
    format: &FormatSpec,
    input_w: u32,
    input_h: u32,
    fonts_dir: Option<&Path>,
) -> Vec<String> {
    let vf = build_vf_chain(ass_path, format, input_w, input_h, fonts_dir);
    vec![
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().into_owned(),
        "-vf".to_string(),
        vf,
        "-c:v".to_string(),
        "h264_videotoolbox".to_string(),
        "-c:a".to_string(),
        "copy".to_string(),
        output.to_string_lossy().into_owned(),
    ]
}

pub fn run_burn(
    ffmpeg_path: &Path,
    input: &Path,
    ass_path: &Path,
    folder: &Path,
    stem: &str,
    format: &FormatSpec,
    input_w: u32,
    input_h: u32,
    fonts_dir: Option<&Path>,
) -> Result<PathBuf, StageError> {
    let output = burn_output_path(folder, stem, format.slug);
    let args = build_burn_args(input, ass_path, &output, format, input_w, input_h, fonts_dir);

    let result = Command::new(ffmpeg_path).args(&args).output().map_err(|e| StageError {
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
    use crate::pipeline::types::OutputFormat;
    use std::path::Path;

    #[test]
    fn burn_output_path_appends_slug() {
        let folder = Path::new("/exports/abc");
        assert_eq!(
            burn_output_path(folder, "myvideo", "ytshort"),
            Path::new("/exports/abc/myvideo_ytshort.mp4")
        );
    }

    #[test]
    fn burn_output_path_captioned_default() {
        let folder = Path::new("/exports/abc");
        assert_eq!(
            burn_output_path(folder, "myvideo", "captioned"),
            Path::new("/exports/abc/myvideo_captioned.mp4")
        );
    }

    #[test]
    fn burn_output_path_uses_folder() {
        let folder = Path::new("/tmp/export");
        let out = burn_output_path(folder, "test", "square");
        assert_eq!(out.parent().unwrap(), Path::new("/tmp/export"));
    }

    #[test]
    fn center_crop_16_9_to_9_16() {
        // 1920x1080 → 9:16 target
        let (cw, ch) = center_crop_dims(1920, 1080, 1080, 1920);
        // input wider → crop width; crop_h = 1080, crop_w = 1080 * 1080 / 1920 = 607 → 606 (even)
        assert_eq!(ch, 1080);
        assert_eq!(cw, 606);
    }

    #[test]
    fn center_crop_9_16_to_4_5() {
        // 1080x1920 → 4:5 target (1080x1350)
        // input aspect = 0.5625, target = 0.8 → input is narrower → crop height
        let (cw, ch) = center_crop_dims(1080, 1920, 1080, 1350);
        assert_eq!(cw, 1080);
        // crop_h = 1080 * 1350 / 1080 = 1350
        assert_eq!(ch, 1350);
    }

    #[test]
    fn center_crop_matches_target_aspect_returns_full() {
        // 1920x1080 → 16:9 target at same aspect → full frame
        let (cw, ch) = center_crop_dims(1920, 1080, 1920, 1080);
        assert_eq!((cw, ch), (1920, 1080));
    }

    #[test]
    fn vf_chain_unchanged_has_only_ass() {
        let spec = OutputFormat::Unchanged.spec(1920, 1080);
        let vf = build_vf_chain(Path::new("/tmp/x.ass"), &spec, 1920, 1080, None);
        assert_eq!(vf, "ass=/tmp/x.ass");
    }

    #[test]
    fn vf_chain_with_fontsdir() {
        let spec = OutputFormat::Unchanged.spec(1920, 1080);
        let vf = build_vf_chain(
            Path::new("/tmp/x.ass"),
            &spec,
            1920,
            1080,
            Some(Path::new("/app/fonts")),
        );
        assert_eq!(vf, "ass=/tmp/x.ass:fontsdir=/app/fonts");
    }

    #[test]
    fn vf_chain_preset_has_crop_scale_ass() {
        let spec = OutputFormat::YoutubeShort.spec(0, 0);
        let vf = build_vf_chain(Path::new("/tmp/x.ass"), &spec, 1920, 1080, None);
        assert!(vf.starts_with("crop="));
        assert!(vf.contains("scale=1080:1920"));
        assert!(vf.contains("ass=/tmp/x.ass"));
    }

    #[test]
    fn build_burn_args_contains_videotoolbox() {
        let spec = OutputFormat::Square.spec(0, 0);
        let args = build_burn_args(
            Path::new("/tmp/video.mp4"),
            Path::new("/tmp/video.ass"),
            Path::new("/tmp/video_square.mp4"),
            &spec,
            1920,
            1080,
            None,
        );
        assert!(args.contains(&"h264_videotoolbox".to_string()));
        assert!(args.iter().any(|a| a.contains("ass=")));
        assert_eq!(args.last().unwrap(), "/tmp/video_square.mp4");
    }

    #[test]
    fn build_burn_args_audio_copy() {
        let spec = OutputFormat::Unchanged.spec(1920, 1080);
        let args = build_burn_args(
            Path::new("/tmp/video.mp4"),
            Path::new("/tmp/video.ass"),
            Path::new("/tmp/video_captioned.mp4"),
            &spec,
            1920,
            1080,
            None,
        );
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
        let spec = OutputFormat::Unchanged.spec(1920, 1080);
        let folder = root.join("test-artifacts");
        let result = run_burn(Path::new("ffmpeg"), &input, &ass, &folder, "sample", &spec, 1920, 1080, None);
        assert!(result.is_ok(), "burn failed: {:?}", result.err());
    }
}
