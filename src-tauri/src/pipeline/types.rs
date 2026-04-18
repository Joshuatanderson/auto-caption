use serde::{Deserialize, Serialize};

// Raw JSON types from `whisper-cli --output-json --output-words`
// ⚠️ Field names must match actual whisper-cli output — verify against a real run
//    if parsing fails. Key uncertainty: `tokens` vs `words`, `p` vs `probability`.

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WTimestamps {
    pub from: String, // "HH:MM:SS,mmm"
    pub to: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WOffsets {
    pub from: i64, // milliseconds
    pub to: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhisperToken {
    pub text: String,
    pub timestamps: WTimestamps,
    pub offsets: WOffsets,
    pub id: i64,
    pub p: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhisperSegment {
    pub timestamps: WTimestamps,
    pub offsets: WOffsets,
    pub text: String,
    pub tokens: Vec<WhisperToken>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhisperOutput {
    pub transcription: Vec<WhisperSegment>,
}

/// Normalized word, derived from WhisperToken after filtering noise tokens.
#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub prob: f64,
}

/// A phrase: a group of words with one accented word.
#[derive(Debug, Clone)]
pub struct Phrase {
    pub words: Vec<Word>,
    pub accent_index: usize,
}

#[derive(Debug, Serialize)]
pub struct StageError {
    pub stage: String,
    pub message: String,
    pub stderr: Option<String>,
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.stage, self.message)
    }
}

/// Style parameters for ASS subtitle generation.
#[derive(Debug, Clone)]
pub struct AssStyle {
    pub font_name: String,
    pub font_size: u32,
    pub primary_color: String,  // ASS hex: &H00FFFFFF (AABBGGRR)
    pub accent_color: String,   // &H0000FFFF (yellow)
    pub outline_color: String,
    pub outline_width: f32,
    pub margin_v: u32,
    pub words_per_phrase: usize,
}

impl Default for AssStyle {
    fn default() -> Self {
        Self {
            font_name: "Arial".to_string(),
            font_size: 72,
            primary_color: "&H00FFFFFF".to_string(),
            accent_color: "&H0000FFFF".to_string(),
            outline_color: "&H00000000".to_string(),
            outline_width: 3.0,
            margin_v: 80,
            words_per_phrase: 5,
        }
    }
}
