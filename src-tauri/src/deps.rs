//! Dependency resolver for the external binaries and model file that the
//! pipeline shells out to. The bundled `.app` inherits a bare `$PATH` from
//! Finder that doesn't include `/opt/homebrew/bin`, so `Command::new("ffmpeg")`
//! fails with `os error 2` in production even though it works fine in
//! `tauri dev`. We resolve absolute paths here, cache them in SQLite, and
//! re-probe on miss.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::Serialize;

/// The four things the pipeline can't run without.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dep {
    Ffmpeg,
    Ffprobe,
    WhisperCli,
    WhisperModel,
}

impl Dep {
    pub const ALL: &'static [Dep] = &[
        Self::Ffmpeg,
        Self::Ffprobe,
        Self::WhisperCli,
        Self::WhisperModel,
    ];

    /// DB key. Stable — changing it invalidates existing caches.
    pub fn key(self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::Ffprobe => "ffprobe",
            Self::WhisperCli => "whisper-cli",
            Self::WhisperModel => "whisper-model",
        }
    }

    /// Human-readable label for UI and error messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::Ffprobe => "ffprobe",
            Self::WhisperCli => "whisper-cli",
            Self::WhisperModel => "Whisper large-v3-turbo model",
        }
    }

    fn is_binary(self) -> bool {
        !matches!(self, Self::WhisperModel)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DepStatus {
    pub key: String,
    pub label: String,
    pub found: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DepReport {
    pub statuses: Vec<DepStatus>,
    /// Labels of missing deps, in the enum's declaration order.
    pub missing: Vec<String>,
    /// Copy-pasteable install instructions aimed at Claude Code / similar.
    /// Empty when nothing is missing.
    pub install_prompt: String,
}

/// Resolved paths for every dep. Only constructable when all four are present.
pub struct ToolPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub whisper_cli: PathBuf,
    pub whisper_model: PathBuf,
}

// --------------------------------------------------------------------------
// Search

/// Extra directories to probe beyond `$PATH`. Covers Apple Silicon Homebrew,
/// Intel Homebrew, MacPorts, system bins, and common user-local install dirs.
fn extra_prefixes() -> Vec<PathBuf> {
    let mut out = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/local/bin"),
        PathBuf::from("/usr/bin"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        out.push(home.join(".local/bin"));
        out.push(home.join("bin"));
    }
    out
}

fn path_env_entries() -> Vec<PathBuf> {
    match std::env::var_os("PATH") {
        Some(p) => std::env::split_paths(&p).collect(),
        None => Vec::new(),
    }
}

fn is_runnable_file(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(p) {
            Ok(m) => m.is_file() && (m.permissions().mode() & 0o111 != 0),
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

fn search_binary(name: &str) -> Option<PathBuf> {
    for dir in path_env_entries().iter().chain(extra_prefixes().iter()) {
        let candidate = dir.join(name);
        if is_runnable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Candidate locations for the Whisper model. `AUTOCAP_WHISPER_MODEL` remains
/// the documented escape hatch so power users can point at a model outside
/// the default layout without a rebuild.
fn model_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("AUTOCAP_WHISPER_MODEL") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            out.push(PathBuf::from(trimmed));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        out.push(home.join(".local/models/whisper/large-v3-turbo.bin"));
        out.push(home.join("Library/Application Support/autocap/models/large-v3-turbo.bin"));
    }
    out.push(PathBuf::from(
        "/opt/homebrew/share/whisper-cpp/models/large-v3-turbo.bin",
    ));
    out.push(PathBuf::from(
        "/usr/local/share/whisper-cpp/models/large-v3-turbo.bin",
    ));
    out
}

fn search_model() -> Option<PathBuf> {
    model_candidates().into_iter().find(|p| p.is_file())
}

// --------------------------------------------------------------------------
// Cache

fn cached_path(conn: &Connection, dep: Dep) -> Option<PathBuf> {
    let row: rusqlite::Result<String> = conn.query_row(
        "SELECT path FROM dep_paths WHERE name = ?1",
        params![dep.key()],
        |r| r.get(0),
    );
    let path = PathBuf::from(row.ok()?);
    if dep.is_binary() {
        if is_runnable_file(&path) { Some(path) } else { None }
    } else if path.is_file() {
        Some(path)
    } else {
        None
    }
}

fn store_path(conn: &Connection, dep: Dep, path: &Path) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO dep_paths (name, path) VALUES (?1, ?2)",
        params![dep.key(), path.to_string_lossy().as_ref()],
    );
}

fn forget_path(conn: &Connection, dep: Dep) {
    let _ = conn.execute("DELETE FROM dep_paths WHERE name = ?1", params![dep.key()]);
}

// --------------------------------------------------------------------------
// Public API

/// Resolves a single dep. Trusts the cache if the cached path still resolves
/// to a runnable/readable file; otherwise re-probes and updates the cache.
pub fn resolve(conn: &Connection, dep: Dep) -> Option<PathBuf> {
    if let Some(p) = cached_path(conn, dep) {
        return Some(p);
    }
    let found = match dep {
        Dep::Ffmpeg => search_binary("ffmpeg"),
        Dep::Ffprobe => search_binary("ffprobe"),
        Dep::WhisperCli => search_binary("whisper-cli"),
        Dep::WhisperModel => search_model(),
    };
    match &found {
        Some(p) => store_path(conn, dep, p),
        None => forget_path(conn, dep),
    }
    found
}

/// Resolves every dep and produces a report suitable for the frontend check.
pub fn check_all(conn: &Connection) -> DepReport {
    let statuses: Vec<DepStatus> = Dep::ALL
        .iter()
        .map(|&dep| {
            let path = resolve(conn, dep);
            DepStatus {
                key: dep.key().to_string(),
                label: dep.label().to_string(),
                found: path.is_some(),
                path: path.map(|p| p.to_string_lossy().into_owned()),
            }
        })
        .collect();
    let missing: Vec<String> = statuses
        .iter()
        .filter(|s| !s.found)
        .map(|s| s.label.clone())
        .collect();
    let missing_deps: Vec<Dep> = Dep::ALL
        .iter()
        .copied()
        .filter(|d| statuses.iter().any(|s| s.key == d.key() && !s.found))
        .collect();
    let install_prompt = build_install_prompt(&missing_deps);
    DepReport { statuses, missing, install_prompt }
}

/// Convenience for the pipeline: either all four paths, or a report of what's
/// missing so the caller can surface the same install prompt.
pub fn resolve_tools(conn: &Connection) -> Result<ToolPaths, DepReport> {
    let report = check_all(conn);
    if !report.missing.is_empty() {
        return Err(report);
    }
    let get = |dep: Dep| -> PathBuf {
        report
            .statuses
            .iter()
            .find(|s| s.key == dep.key())
            .and_then(|s| s.path.clone())
            .map(PathBuf::from)
            .expect("missing list is empty so every status must have a path")
    };
    Ok(ToolPaths {
        ffmpeg: get(Dep::Ffmpeg),
        ffprobe: get(Dep::Ffprobe),
        whisper_cli: get(Dep::WhisperCli),
        whisper_model: get(Dep::WhisperModel),
    })
}

/// Produces a prompt designed to be pasted into Claude Code (or any AI coding
/// assistant). We always include the full install recipe even when only one
/// dep is missing — a non-technical friend shouldn't have to reason about
/// which subset matters.
fn build_install_prompt(missing: &[Dep]) -> String {
    if missing.is_empty() {
        return String::new();
    }
    let labels: Vec<&str> = missing.iter().map(|d| d.label()).collect();
    format!(
        "I'm running Captioner (a local macOS video caption tool) and it's missing \
these dependencies: {missing}.\n\
\n\
Please install them for me. Commands (macOS, Apple Silicon or Intel):\n\
\n\
```bash\n\
# 1. Install Homebrew if it isn't already installed\n\
if ! command -v brew >/dev/null 2>&1; then\n\
  /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"\n\
fi\n\
\n\
# 2. Install ffmpeg (brings in libass + fontconfig) and whisper-cli\n\
brew install ffmpeg whisper-cpp\n\
\n\
# 3. Download the Whisper large-v3-turbo model (~1.5 GB)\n\
mkdir -p \"$HOME/.local/models/whisper\"\n\
if [ ! -f \"$HOME/.local/models/whisper/large-v3-turbo.bin\" ]; then\n\
  curl -L --fail \\\n\
    -o \"$HOME/.local/models/whisper/large-v3-turbo.bin\" \\\n\
    https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin\n\
fi\n\
```\n\
\n\
After install, reopen Captioner (or click \"Re-check dependencies\"). It will \
auto-detect the installed paths and cache them, so this check only runs once.\n",
        missing = labels.join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_keys_are_stable_identifiers() {
        assert_eq!(Dep::Ffmpeg.key(), "ffmpeg");
        assert_eq!(Dep::Ffprobe.key(), "ffprobe");
        assert_eq!(Dep::WhisperCli.key(), "whisper-cli");
        assert_eq!(Dep::WhisperModel.key(), "whisper-model");
    }

    #[test]
    fn install_prompt_empty_when_nothing_missing() {
        assert_eq!(build_install_prompt(&[]), "");
    }

    #[test]
    fn install_prompt_lists_missing_deps_by_label() {
        let prompt = build_install_prompt(&[Dep::Ffmpeg, Dep::WhisperModel]);
        assert!(prompt.contains("ffmpeg"));
        assert!(prompt.contains("Whisper large-v3-turbo model"));
        assert!(prompt.contains("brew install ffmpeg whisper-cpp"));
    }

    #[test]
    fn extra_prefixes_includes_apple_silicon_homebrew() {
        let prefixes = extra_prefixes();
        assert!(prefixes.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert!(prefixes.contains(&PathBuf::from("/usr/local/bin")));
    }
}
