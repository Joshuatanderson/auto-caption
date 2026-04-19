use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::db;
use crate::pipeline;
use crate::pipeline::types::{AssStyle, OutputFormat, StageError};

/// Name of the subfolder inside each per-action export folder that holds
/// creation artifacts (wav, whisper json, ass). Final burned MP4s sit at the
/// top of the export folder so the user sees outputs first, not intermediates.
const ARTIFACTS_SUBDIR: &str = "artifacts";

/// Event name the frontend listens on. One event is emitted each time the
/// pipeline transitions into a new stage.
const PROGRESS_EVENT: &str = "pipeline-progress";

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

fn artifacts_dir_of(folder: &Path) -> PathBuf {
    folder.join(ARTIFACTS_SUBDIR)
}

/// Creates a fresh per-action export folder plus its `artifacts/` subfolder.
///
/// Root selection:
///   - `configured_root = Some(p)` and `p` exists → `<p>/<stem>_export_<secs>/`
///   - `configured_root = Some(p)` but `p` does NOT exist → fail loud (per
///     CLAUDE.md): the user set a path and expects artifacts there; silently
///     writing next to the input would re-clutter their source tree without
///     warning.
///   - `configured_root = None` → `<input_parent>/<stem>_export_<secs>/`
fn make_export_folder(
    stage: &str,
    input: &Path,
    configured_root: Option<&Path>,
) -> Result<PathBuf, StageError> {
    let root = match configured_root {
        Some(p) => {
            if !p.exists() {
                return Err(StageError {
                    stage: stage.to_string(),
                    message: format!(
                        "Configured output directory does not exist: {}. \
                         Update it in settings or clear it to use the video folder.",
                        p.display()
                    ),
                    stderr: None,
                });
            }
            p.to_path_buf()
        }
        None => input.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    let stem = stem_of(input);
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let folder = root.join(format!("{stem}_export_{secs}"));
    std::fs::create_dir_all(artifacts_dir_of(&folder)).map_err(|e| StageError {
        stage: stage.to_string(),
        message: format!("Failed to create export folder: {e}"),
        stderr: None,
    })?;
    Ok(folder)
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PipelineStage {
    Audio,
    Transcribe,
    Ass,
    Burn,
}

/// Per-stage progress notification. One event fires as each stage starts
/// (`running`). The completion of the whole pipeline is signaled by the
/// command's return value rather than a "done" event.
#[derive(Debug, Clone, Serialize)]
pub struct PipelineProgress {
    pub stage: PipelineStage,
}

#[derive(Serialize, Deserialize)]
pub struct PipelineResult {
    pub folder: String,
    pub files: Vec<String>,
}

fn emit_stage(app: &tauri::AppHandle, stage: PipelineStage) {
    // We intentionally swallow emit errors: a missing listener must not fail
    // the pipeline itself.
    let _ = app.emit(PROGRESS_EVENT, PipelineProgress { stage });
}

/// `async fn` is load-bearing: each `spawn_blocking(...).await` between stages
/// yields the Tauri runtime worker, which is what lets `app.emit(...)` events
/// actually reach the webview between stages instead of arriving as a batch
/// once the whole command returns. Keeping this as a sync `fn` meant the
/// webview never painted `pipeline-progress` events until after all four
/// stages had run.
#[tauri::command]
pub async fn run_pipeline(
    app: tauri::AppHandle,
    input_path: String,
    formats: Vec<OutputFormat>,
    state: tauri::State<'_, db::DbState>,
) -> Result<PipelineResult, String> {
    if formats.is_empty() {
        return Err(stage_err("run_pipeline", "Select at least one output format"));
    }

    let input = PathBuf::from(input_path);

    // Single DB read for everything we need. Must release the lock before any
    // `.await` (std::sync::Mutex guard is !Send).
    let (configured_output, colors, position) = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        (
            db::current_output_dir(&conn),
            db::current_ass_style(&conn),
            db::current_caption_position(&conn),
        )
    };

    let (iw, ih) = pipeline::probe::probe_dimensions(&input).map_err(err)?;

    let folder =
        make_export_folder("run_pipeline", &input, configured_output.as_deref()).map_err(err)?;
    let artifacts = artifacts_dir_of(&folder);
    let stem = stem_of(&input);

    // ---- Audio ----
    emit_stage(&app, PipelineStage::Audio);
    let audio_path = {
        let input = input.clone();
        let artifacts = artifacts.clone();
        tauri::async_runtime::spawn_blocking(move || {
            pipeline::audio::run_extract_audio(&input, Some(&artifacts))
        })
        .await
        .map_err(|e| stage_err("extract_audio", format!("task join error: {e}")))?
        .map_err(err)?
    };

    // ---- Transcribe ----
    emit_stage(&app, PipelineStage::Transcribe);
    let transcript = {
        let audio_path = audio_path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            pipeline::transcribe::run_transcribe(&audio_path)
        })
        .await
        .map_err(|e| stage_err("transcribe", format!("task join error: {e}")))?
        .map_err(err)?
    };

    // ---- Generate ASS (in-memory + small file writes; stays inline) ----
    emit_stage(&app, PipelineStage::Ass);
    for format in &formats {
        let spec = format.spec(iw, ih);
        let style = AssStyle {
            primary_color: colors.primary_color.clone(),
            accent_color: colors.accent_color.clone(),
            font_size: spec.font_size,
            margin_v: spec.margin_v,
            position,
            ..AssStyle::default()
        };
        let content = pipeline::ass::generate_ass(&transcript, &style, spec.width, spec.height);
        pipeline::ass::write_ass_file(&artifacts, &stem, spec.slug, &content).map_err(err)?;
    }

    // ---- Burn (one mp4 per format) ----
    emit_stage(&app, PipelineStage::Burn);
    let fonts_dir = app.path().resource_dir().ok().map(|p| p.join("fonts"));
    let mut files = Vec::with_capacity(formats.len());
    for format in &formats {
        let spec = format.spec(iw, ih);
        let ass_path = artifacts.join(format!("{stem}_{slug}.ass", slug = spec.slug));
        let out = {
            let input = input.clone();
            let folder = folder.clone();
            let stem = stem.clone();
            let fonts_dir = fonts_dir.clone();
            tauri::async_runtime::spawn_blocking(move || {
                pipeline::burn::run_burn(
                    &input,
                    &ass_path,
                    &folder,
                    &stem,
                    &spec,
                    iw,
                    ih,
                    fonts_dir.as_deref(),
                )
            })
            .await
            .map_err(|e| stage_err("burn", format!("task join error: {e}")))?
            .map_err(err)?
        };
        files.push(out.to_string_lossy().into_owned());
    }

    Ok(PipelineResult {
        folder: folder.to_string_lossy().into_owned(),
        files,
    })
}
