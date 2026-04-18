use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::db;
use crate::pipeline;
use crate::pipeline::types::{OutputFormat, StageError, WhisperOutput};

fn err(e: StageError) -> String {
    serde_json::to_string(&e).unwrap_or_else(|_| e.to_string())
}

fn stage_err(stage: &str, message: impl Into<String>) -> String {
    err(StageError {
        stage: stage.to_string(),
        message: message.into(),
        stderr: None,
    })
}

fn stem_of(input: &Path) -> String {
    input.file_stem().unwrap_or_default().to_string_lossy().into_owned()
}

/// Creates a fresh per-export folder next to the input: <input_dir>/<stem>_export_<unix_secs>/.
fn make_export_folder(input: &Path) -> Result<PathBuf, StageError> {
    let parent = input.parent().unwrap_or(Path::new("."));
    let stem = stem_of(input);
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let folder = parent.join(format!("{stem}_export_{secs}"));
    std::fs::create_dir_all(&folder).map_err(|e| StageError {
        stage: "generate_ass".to_string(),
        message: format!("Failed to create export folder: {e}"),
        stderr: None,
    })?;
    Ok(folder)
}

#[tauri::command]
pub fn extract_audio(input_path: String) -> Result<String, String> {
    pipeline::audio::run_extract_audio(&PathBuf::from(input_path))
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(err)
}

#[tauri::command]
pub fn transcribe(wav_path: String) -> Result<WhisperOutput, String> {
    pipeline::transcribe::run_transcribe(&PathBuf::from(wav_path)).map_err(err)
}

#[derive(Serialize, Deserialize)]
pub struct GenerateResult {
    pub folder: String,
    pub formats: Vec<OutputFormat>,
}

#[tauri::command]
pub fn generate_ass(
    input_path: String,
    transcript: WhisperOutput,
    formats: Vec<OutputFormat>,
    state: tauri::State<'_, db::DbState>,
) -> Result<GenerateResult, String> {
    if formats.is_empty() {
        return Err(stage_err("generate_ass", "Select at least one output format"));
    }
    let input = PathBuf::from(input_path);
    let (iw, ih) = pipeline::probe::probe_dimensions(&input).map_err(err)?;
    let folder = make_export_folder(&input).map_err(err)?;
    let stem = stem_of(&input);

    let colors = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        db::current_ass_style(&conn)
    };

    for format in &formats {
        let spec = format.spec(iw, ih);
        let mut style = pipeline::types::AssStyle::default();
        style.primary_color = colors.primary_color.clone();
        style.accent_color = colors.accent_color.clone();
        style.font_size = spec.font_size;
        style.margin_v = spec.margin_v;
        let content = pipeline::ass::generate_ass(&transcript, &style, spec.width, spec.height);
        pipeline::ass::write_ass_file(&folder, &stem, spec.slug, &content).map_err(err)?;
    }

    Ok(GenerateResult {
        folder: folder.to_string_lossy().into_owned(),
        formats,
    })
}

#[derive(Serialize, Deserialize)]
pub struct BurnResult {
    pub folder: String,
    pub files: Vec<String>,
}

#[tauri::command]
pub fn burn_captions(
    input_path: String,
    folder: String,
    formats: Vec<OutputFormat>,
) -> Result<BurnResult, String> {
    if formats.is_empty() {
        return Err(stage_err("burn_captions", "Select at least one output format"));
    }
    let input = PathBuf::from(input_path);
    let folder_path = PathBuf::from(&folder);
    let stem = stem_of(&input);
    let (iw, ih) = pipeline::probe::probe_dimensions(&input).map_err(err)?;

    let mut files = Vec::with_capacity(formats.len());
    for format in &formats {
        let spec = format.spec(iw, ih);
        let ass_path = folder_path.join(format!("{stem}_{slug}.ass", slug = spec.slug));
        let out = pipeline::burn::run_burn(&input, &ass_path, &folder_path, &stem, &spec, iw, ih)
            .map_err(err)?;
        files.push(out.to_string_lossy().into_owned());
    }
    Ok(BurnResult { folder, files })
}
