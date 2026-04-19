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
}

/// A group of consecutive words rendered together as one caption event.
/// Per-word highlighting is driven by each word's `start_ms`/`end_ms`.
#[derive(Debug, Clone)]
pub struct Phrase {
    pub words: Vec<Word>,
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

/// Vertical placement of captions. Maps to ASS `Alignment` numpad values
/// (Top=8, Middle=5, Bottom=2). `margin_v` is the distance from the reference
/// edge for Top/Bottom and is effectively ignored for Middle (libass centers).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CaptionPosition {
    Top,
    Middle,
    #[default]
    Bottom,
}

/// Style parameters for ASS subtitle generation.
#[derive(Debug, Clone)]
pub struct AssStyle {
    pub font_name: String,
    pub font_size: u32,
    pub primary_color: String,  // ASS hex: &H00FFFFFF (AABBGGRR)
    pub accent_color: String,   // active/highlight color the currently-spoken word flashes to (default &H0000FFFF yellow)
    pub outline_color: String,
    pub outline_width: f32,
    pub margin_v: u32,
    pub position: CaptionPosition,
    pub words_per_phrase: usize,
    /// Lead-in before the first word of each phrase lights up, in milliseconds.
    /// Gives the viewer's eye time to find the new caption before any word
    /// flashes to the accent color. Without this, the first word feels "early"
    /// at phrase boundaries because the text pops in and highlights at the
    /// same instant.
    pub first_word_lead_in_ms: u32,
}

impl Default for AssStyle {
    fn default() -> Self {
        Self {
            font_name: "Noto Sans".to_string(),
            font_size: 72,
            primary_color: "&H00FFFFFF".to_string(),
            accent_color: "&H0000FFFF".to_string(),
            outline_color: "&H00000000".to_string(),
            outline_width: 3.0,
            margin_v: 80,
            position: CaptionPosition::Bottom,
            words_per_phrase: 5,
            first_word_lead_in_ms: 100,
        }
    }
}

/// Target output format the user selects before burn. `Unchanged` preserves
/// input dimensions; presets crop+scale to platform-native sizes.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    Unchanged,
    YoutubeShort,   // 1080x1920, 9:16
    LinkedinShort,  // 1080x1350, 4:5 — LinkedIn's highest-engagement feed crop
    Square,         // 1080x1080, 1:1
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Unchanged
    }
}

/// Resolved dimensions + style scale for a given OutputFormat, filename slug included.
#[derive(Debug, Clone)]
pub struct FormatSpec {
    pub width: u32,
    pub height: u32,
    pub font_size: u32,
    pub margin_v: u32,
    pub slug: &'static str,
}

impl OutputFormat {
    /// Resolves to a FormatSpec. `input_w`/`input_h` are only consulted for
    /// `Unchanged`; presets ignore them.
    pub fn spec(&self, input_w: u32, input_h: u32) -> FormatSpec {
        match self {
            // Scale font/margin from 1080p defaults (72px font @ 1080h ≈ 6.67%,
            // 80px margin @ 1080h ≈ 7.4%) so vertical inputs don't get tiny captions.
            OutputFormat::Unchanged => FormatSpec {
                width: input_w,
                height: input_h,
                font_size: ((input_h as f32) * 0.0667).round().max(16.0) as u32,
                margin_v: ((input_h as f32) * 0.074).round() as u32,
                slug: "captioned",
            },
            OutputFormat::YoutubeShort => FormatSpec {
                width: 1080,
                height: 1920,
                font_size: 80,
                margin_v: 400,
                slug: "ytshort",
            },
            OutputFormat::LinkedinShort => FormatSpec {
                width: 1080,
                height: 1350,
                font_size: 72,
                margin_v: 200,
                slug: "lishort",
            },
            OutputFormat::Square => FormatSpec {
                width: 1080,
                height: 1080,
                font_size: 64,
                margin_v: 100,
                slug: "square",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_unchanged_matches_input() {
        let s = OutputFormat::Unchanged.spec(1920, 1080);
        assert_eq!((s.width, s.height), (1920, 1080));
        assert_eq!(s.slug, "captioned");
        // at h=1080, font_size ≈ 72, margin_v ≈ 80
        assert!((70..=74).contains(&s.font_size));
        assert!((78..=82).contains(&s.margin_v));
    }

    #[test]
    fn spec_unchanged_scales_for_vertical() {
        let s = OutputFormat::Unchanged.spec(1080, 1920);
        assert_eq!((s.width, s.height), (1080, 1920));
        assert!(s.font_size > 100, "tall input should get larger font, got {}", s.font_size);
    }

    #[test]
    fn spec_presets_have_expected_dims() {
        assert_eq!(OutputFormat::YoutubeShort.spec(0, 0).width, 1080);
        assert_eq!(OutputFormat::YoutubeShort.spec(0, 0).height, 1920);
        assert_eq!(OutputFormat::LinkedinShort.spec(0, 0).width, 1080);
        assert_eq!(OutputFormat::LinkedinShort.spec(0, 0).height, 1350);
        assert_eq!(OutputFormat::Square.spec(0, 0).width, 1080);
        assert_eq!(OutputFormat::Square.spec(0, 0).height, 1080);
    }

    #[test]
    fn output_format_serde_kebab_case() {
        let s = serde_json::to_string(&OutputFormat::YoutubeShort).unwrap();
        assert_eq!(s, "\"youtube-short\"");
        let parsed: OutputFormat = serde_json::from_str("\"linkedin-short\"").unwrap();
        assert_eq!(parsed, OutputFormat::LinkedinShort);
    }
}
