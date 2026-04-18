use std::path::{Path, PathBuf};

use crate::pipeline::types::{AssStyle, Phrase, StageError, WhisperOutput, Word};

/// Extracts word-level units from whisper's sub-word BPE tokens, filtering noise.
///
/// Whisper tokenizes with a leading-space convention: a token that begins with
/// whitespace starts a new word, a token without leading whitespace continues
/// the previous word. Contractions and punctuation ride along as continuations:
///   "don't" → " don" + "'t"
///   "you're" → " you" + "'re"
///   "Hello," → " Hello" + ","
/// We must inspect the raw text *before* trimming, otherwise the word-boundary
/// signal is lost and later join-by-space produces "don 't" and "Hello ,".
///
/// Noise tokens ([_BEG_], [_TT_N], <|...|>) are dropped.
pub fn flatten_words(output: &WhisperOutput) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    for seg in &output.transcription {
        for t in &seg.tokens {
            let trimmed = t.text.trim();
            if trimmed.is_empty() || trimmed.starts_with('[') || trimmed.starts_with('<') {
                continue;
            }
            let starts_new_word = words.is_empty() || t.text.starts_with(char::is_whitespace);
            if starts_new_word {
                words.push(Word {
                    text: trimmed.to_string(),
                    start_ms: t.offsets.from,
                    end_ms: t.offsets.to,
                    prob: t.p,
                });
            } else {
                let last = words.last_mut().unwrap();
                last.text.push_str(trimmed);
                last.end_ms = t.offsets.to;
            }
        }
    }
    words
}

/// Groups words into phrases of approximately `target_size` words.
pub fn words_to_phrases(words: &[Word], target_size: usize) -> Vec<Phrase> {
    if words.is_empty() {
        return vec![];
    }
    words
        .chunks(target_size)
        .map(|chunk| {
            let accent_index = chunk
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.prob.partial_cmp(&b.prob).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);
            Phrase { words: chunk.to_vec(), accent_index }
        })
        .collect()
}

/// Formats seconds as ASS timestamp: `h:mm:ss.cs` (centiseconds). Pure function.
pub fn seconds_to_ass_timestamp(secs: f64) -> String {
    let total_cs = (secs * 100.0).round() as u64;
    let cs = total_cs % 100;
    let total_s = total_cs / 100;
    let s = total_s % 60;
    let m = (total_s / 60) % 60;
    let h = total_s / 3600;
    format!("{h}:{m:02}:{s:02}.{cs:02}")
}

fn ms_to_ass(ms: i64) -> String {
    seconds_to_ass_timestamp(ms as f64 / 1000.0)
}

/// Emits one word of a phrase wrapped in `\t` transforms that flash its color
/// to the accent color during its active time window. Times are milliseconds
/// relative to the enclosing Dialogue event's Start.
///
/// The first word of a phrase gets a small lead-in delay (from
/// `style.first_word_lead_in_ms`) so the viewer can orient to the new caption
/// text before it highlights. Clamped so it never pushes past the word's end.
fn word_span(word: &Word, phrase_start_ms: i64, style: &AssStyle, is_first_in_phrase: bool) -> String {
    let natural_start = (word.start_ms - phrase_start_ms).max(0);
    let rel_end = (word.end_ms - phrase_start_ms).max(natural_start);
    let rel_start = if is_first_in_phrase {
        (natural_start + style.first_word_lead_in_ms as i64).min(rel_end)
    } else {
        natural_start
    };
    format!(
        "{{\\t({rs},{rs},\\c{accent})\\t({re},{re},\\c{primary})}}{text}",
        rs = rel_start,
        re = rel_end,
        accent = style.accent_color,
        primary = style.primary_color,
        text = word.text,
    )
}

/// Renders a single phrase as one ASS Dialogue line.
/// Each word is wrapped in per-word `\t` transforms so the highlight follows
/// the currently-spoken word as playback progresses.
pub fn phrase_to_ass_event(phrase: &Phrase, style: &AssStyle) -> String {
    if phrase.words.is_empty() {
        return String::new();
    }
    let phrase_start_ms = phrase.words.first().unwrap().start_ms;
    let start = ms_to_ass(phrase_start_ms);
    let end = ms_to_ass(phrase.words.last().unwrap().end_ms);

    let text: Vec<String> = phrase
        .words
        .iter()
        .enumerate()
        .map(|(i, w)| word_span(w, phrase_start_ms, style, i == 0))
        .collect();

    format!("Dialogue: 0,{start},{end},Default,,0,0,0,,{}\n", text.join(" "))
}

/// Builds the ASS header block. Pure function.
pub fn build_ass_header(style: &AssStyle) -> String {
    format!(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: 1920\n\
         PlayResY: 1080\n\
         ScaledBorderAndShadow: yes\n\
         \n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, \
                 Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, \
                 Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Default,{font},{size},{primary},&H000000FF,{outline},&H00000000,\
                -1,0,0,0,100,100,0,0,1,{outline_w:.1},0,2,10,10,{margin_v},1\n\
         \n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        font = style.font_name,
        size = style.font_size,
        primary = style.primary_color,
        outline = style.outline_color,
        outline_w = style.outline_width,
        margin_v = style.margin_v,
    )
}

/// Converts a WhisperOutput to a complete ASS file string. Fully pure.
pub fn generate_ass(output: &WhisperOutput, style: &AssStyle) -> String {
    let words = flatten_words(output);
    let phrases = words_to_phrases(&words, style.words_per_phrase);
    let mut ass = build_ass_header(style);
    for phrase in &phrases {
        ass.push_str(&phrase_to_ass_event(phrase, style));
    }
    ass
}

pub fn write_ass_file(video_path: &Path, content: &str) -> Result<PathBuf, StageError> {
    let out = video_path.with_extension("ass");
    std::fs::write(&out, content).map_err(|e| StageError {
        stage: "generate_ass".to_string(),
        message: format!("Failed to write .ass file: {e}"),
        stderr: None,
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::types::{AssStyle, WhisperOutput, WhisperSegment, WhisperToken, WOffsets, WTimestamps};

    fn ts(from: &str, to: &str) -> WTimestamps {
        WTimestamps { from: from.to_string(), to: to.to_string() }
    }

    fn off(from: i64, to: i64) -> WOffsets {
        WOffsets { from, to }
    }

    fn tok(text: &str, from_ms: i64, to_ms: i64, prob: f64) -> WhisperToken {
        WhisperToken { text: text.to_string(), timestamps: ts("", ""), offsets: off(from_ms, to_ms), id: 0, p: prob }
    }

    fn make_output(tokens: Vec<WhisperToken>) -> WhisperOutput {
        WhisperOutput {
            transcription: vec![WhisperSegment {
                timestamps: ts("00:00:00,000", "00:00:10,000"),
                offsets: off(0, 10000),
                text: "test".to_string(),
                tokens,
            }],
        }
    }

    // --- seconds_to_ass_timestamp ---

    #[test]
    fn timestamp_zero() {
        assert_eq!(seconds_to_ass_timestamp(0.0), "0:00:00.00");
    }

    #[test]
    fn timestamp_one_minute_plus() {
        assert_eq!(seconds_to_ass_timestamp(61.5), "0:01:01.50");
    }

    #[test]
    fn timestamp_over_one_hour() {
        assert_eq!(seconds_to_ass_timestamp(3661.0), "1:01:01.00");
    }

    #[test]
    fn timestamp_centiseconds() {
        assert_eq!(seconds_to_ass_timestamp(1.05), "0:00:01.05");
    }

    // --- flatten_words ---

    #[test]
    fn flatten_words_filters_special_tokens() {
        let tokens = vec![
            tok("[_BEG_]", 0, 0, 1.0),
            tok(" Hello", 0, 500, 0.9),
            tok("[_TT_50]", 500, 500, 1.0),
            tok(" world", 500, 1000, 0.8),
        ];
        let words = flatten_words(&make_output(tokens));
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[1].text, "world");
    }

    #[test]
    fn flatten_words_trims_leading_space() {
        let tokens = vec![tok(" Hello", 0, 500, 0.9)];
        let words = flatten_words(&make_output(tokens));
        assert_eq!(words[0].text, "Hello");
    }

    #[test]
    fn flatten_words_merges_contractions() {
        // "don't" arrives as two BPE tokens: " don" + "'t"
        let tokens = vec![
            tok(" don", 0, 300, 0.9),
            tok("'t",  300, 400, 0.85),
        ];
        let words = flatten_words(&make_output(tokens));
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "don't");
        assert_eq!(words[0].start_ms, 0);
        assert_eq!(words[0].end_ms, 400);
    }

    #[test]
    fn flatten_words_merges_multiple_contractions_in_sequence() {
        // "you're so don't" → 4 words even though whisper emits 6 tokens
        let tokens = vec![
            tok(" you",  0,    200, 0.9),
            tok("'re",   200,  300, 0.8),
            tok(" so",   300,  500, 0.95),
            tok(" don",  500,  700, 0.9),
            tok("'t",    700,  800, 0.8),
            tok(" stop", 800, 1100, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        assert_eq!(words.len(), 4);
        assert_eq!(words[0].text, "you're");
        assert_eq!(words[1].text, "so");
        assert_eq!(words[2].text, "don't");
        assert_eq!(words[3].text, "stop");
    }

    #[test]
    fn flatten_words_merges_punctuation() {
        // Punctuation has no leading space and should attach to the previous word
        let tokens = vec![
            tok(" Hello", 0,   500, 0.9),
            tok(",",       500, 510, 0.99),
            tok(" world",  510, 900, 0.9),
            tok(".",       900, 910, 0.99),
        ];
        let words = flatten_words(&make_output(tokens));
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello,");
        assert_eq!(words[1].text, "world.");
    }

    #[test]
    fn flatten_words_first_token_has_no_leading_space() {
        // Sometimes whisper's very first token has no leading space; it must still
        // start a new word rather than try to merge into a nonexistent predecessor.
        let tokens = vec![
            tok("Hello", 0, 500, 0.9),
            tok(" world", 500, 1000, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[1].text, "world");
    }

    // --- words_to_phrases ---

    #[test]
    fn phrases_even_split() {
        let tokens: Vec<_> = (0..10).map(|i| tok(" w", i * 100, (i + 1) * 100, 0.9)).collect();
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        assert_eq!(phrases.len(), 2);
        assert_eq!(phrases[0].words.len(), 5);
        assert_eq!(phrases[1].words.len(), 5);
    }

    #[test]
    fn phrases_remainder() {
        let tokens: Vec<_> = (0..7).map(|i| tok(" w", i * 100, (i + 1) * 100, 0.9)).collect();
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        assert_eq!(phrases.len(), 2);
        assert_eq!(phrases[1].words.len(), 2);
    }

    #[test]
    fn phrases_empty_input() {
        let phrases = words_to_phrases(&[], 5);
        assert!(phrases.is_empty());
    }

    #[test]
    fn accent_is_highest_probability() {
        let tokens = vec![
            tok(" low",  0, 500, 0.5),
            tok(" high", 500, 1000, 0.95),
            tok(" mid",  1000, 1500, 0.7),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        assert_eq!(phrases[0].accent_index, 1);
    }

    // --- phrase_to_ass_event ---

    #[test]
    fn every_word_has_timed_color_transform() {
        let tokens = vec![
            tok(" hello", 0, 500, 0.9),
            tok(" world", 500, 1000, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle { first_word_lead_in_ms: 0, ..AssStyle::default() };
        let event = phrase_to_ass_event(&phrases[0], &style);
        assert!(event.starts_with("Dialogue:"));
        assert!(event.contains("hello"));
        assert!(event.contains("world"));
        // With no lead-in, first word's transform fires at t=0 relative to event start
        assert!(event.contains(&format!("\\t(0,0,\\c{})", style.accent_color)));
        // Each word gets its own open + close transform pair → 2 words × 2 transforms = 4 `\t(`
        assert_eq!(event.matches("\\t(").count(), 4);
    }

    #[test]
    fn one_transform_block_per_word() {
        let tokens: Vec<_> = (0..5).map(|i| tok(" w", i * 100, (i + 1) * 100, 0.5)).collect();
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle::default();
        let event = phrase_to_ass_event(&phrases[0], &style);
        // N words → N open transforms (to accent) and N close transforms (to primary)
        let open_count = event.matches(&format!("\\c{}", style.accent_color)).count();
        let close_count = event.matches(&format!("\\c{}", style.primary_color)).count();
        assert_eq!(open_count, 5);
        assert_eq!(close_count, 5);
    }

    #[test]
    fn transform_times_are_relative_to_phrase_start() {
        // Phrase starts at t=5000ms absolute; with lead-in disabled, first word's
        // transform must fire at relative t=0.
        let tokens = vec![
            tok(" first",  5000, 5400, 0.9),
            tok(" second", 5400, 5900, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle { first_word_lead_in_ms: 0, ..AssStyle::default() };
        let event = phrase_to_ass_event(&phrases[0], &style);
        assert!(event.contains(&format!("\\t(0,0,\\c{})", style.accent_color)));
        assert!(event.contains(&format!("\\t(400,400,\\c{})", style.primary_color)));
        assert!(event.contains(&format!("\\t(400,400,\\c{})", style.accent_color)));
        assert!(event.contains(&format!("\\t(900,900,\\c{})", style.primary_color)));
    }

    #[test]
    fn first_word_respects_lead_in() {
        // Default lead-in of 100ms pushes the first word's highlight to t=100,
        // while subsequent words keep their natural timing.
        let tokens = vec![
            tok(" first",  0,   500,  0.9),
            tok(" second", 500, 1000, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle::default();  // first_word_lead_in_ms = 100
        let event = phrase_to_ass_event(&phrases[0], &style);
        // First word: delayed by lead-in
        assert!(event.contains(&format!("\\t(100,100,\\c{})", style.accent_color)));
        // First word still closes at its natural end
        assert!(event.contains(&format!("\\t(500,500,\\c{})", style.primary_color)));
        // Second word unaffected by lead-in
        assert!(event.contains(&format!("\\t(500,500,\\c{})", style.accent_color)));
        assert!(event.contains(&format!("\\t(1000,1000,\\c{})", style.primary_color)));
    }

    #[test]
    fn first_word_lead_in_clamped_when_word_is_shorter() {
        // A very short first word (50ms) can't absorb a 100ms lead-in.
        // The highlight start should clamp to the word's end so rendering stays valid.
        let tokens = vec![
            tok(" short", 0,  50,  0.9),
            tok(" next",  50, 500, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle::default();  // lead-in = 100, word is 50ms
        let event = phrase_to_ass_event(&phrases[0], &style);
        // Clamped: start == end == 50ms (effectively no highlight for this short word)
        assert!(event.contains(&format!("\\t(50,50,\\c{})", style.accent_color)));
        assert!(event.contains(&format!("\\t(50,50,\\c{})", style.primary_color)));
    }

    #[test]
    fn transform_uses_style_colors_not_hardcoded() {
        let tokens = vec![tok(" word", 0, 500, 0.9)];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle {
            accent_color: "&H0000AAAA".to_string(),
            primary_color: "&H00BBBBBB".to_string(),
            ..AssStyle::default()
        };
        let event = phrase_to_ass_event(&phrases[0], &style);
        assert!(event.contains("&H0000AAAA"));
        assert!(event.contains("&H00BBBBBB"));
        // Default yellow must NOT appear when style overrides it
        assert!(!event.contains("&H0000FFFF"));
    }

    // --- build_ass_header ---

    #[test]
    fn header_contains_script_info() {
        let style = AssStyle::default();
        let header = build_ass_header(&style);
        assert!(header.contains("[Script Info]"));
        assert!(header.contains("[V4+ Styles]"));
        assert!(header.contains("[Events]"));
    }

    #[test]
    fn header_contains_style_values() {
        let style = AssStyle::default();
        let header = build_ass_header(&style);
        assert!(header.contains("Arial"));
        assert!(header.contains("72"));
    }

    // --- generate_ass (end-to-end) ---

    #[test]
    fn generate_ass_correct_dialogue_count() {
        // 6 words, target 5 → 2 phrases → 2 Dialogue lines
        let tokens: Vec<_> = (0..6)
            .map(|i| tok(" word", i * 500, (i + 1) * 500, 0.9))
            .collect();
        let output = make_output(tokens);
        let style = AssStyle::default();
        let ass = generate_ass(&output, &style);
        assert_eq!(ass.matches("Dialogue:").count(), 2);
    }

    #[test]
    fn generate_ass_well_formed() {
        let tokens: Vec<_> = (0..5)
            .map(|i| tok(" hello", i * 500, (i + 1) * 500, 0.9))
            .collect();
        let output = make_output(tokens);
        let style = AssStyle::default();
        let ass = generate_ass(&output, &style);
        assert!(ass.starts_with("[Script Info]"));
        assert!(ass.contains("Dialogue:"));
    }
}
