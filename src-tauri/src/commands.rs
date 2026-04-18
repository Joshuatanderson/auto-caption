use std::path::PathBuf;

use crate::db;
use crate::pipeline;
use crate::pipeline::types::WhisperOutput;

fn err(e: pipeline::types::StageError) -> String {
    serde_json::to_string(&e).unwrap_or_else(|_| e.to_string())
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

#[tauri::command]
pub fn generate_ass(
    input_path: String,
    transcript: WhisperOutput,
    state: tauri::State<'_, db::DbState>,
) -> Result<String, String> {
    let mut style = pipeline::types::AssStyle::default();
    {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let colors = db::current_ass_style(&conn);
        style.primary_color = colors.primary_color;
        style.accent_color = colors.accent_color;
    }
    let content = pipeline::ass::generate_ass(&transcript, &style);
    pipeline::ass::write_ass_file(&PathBuf::from(input_path), &content)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(err)
}

#[tauri::command]
pub fn burn_captions(input_path: String, ass_path: String) -> Result<String, String> {
    pipeline::burn::run_burn(&PathBuf::from(input_path), &PathBuf::from(ass_path))
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(err)
}
