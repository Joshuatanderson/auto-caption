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
        .map(|chunk| Phrase { words: chunk.to_vec() })
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

/// Emits one Dialogue line spanning [start_ms, end_ms] showing the whole
/// phrase text, with at most one word statically wrapped in the accent color.
/// `accent_index = None` emits the phrase with no word highlighted (lead-in).
fn build_dialogue_line(
    start_ms: i64,
    end_ms: i64,
    words: &[Word],
    accent_index: Option<usize>,
    style: &AssStyle,
) -> String {
    let start = ms_to_ass(start_ms);
    let end = ms_to_ass(end_ms);
    let text: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if Some(i) == accent_index {
                format!(
                    "{{\\c{accent}}}{}{{\\c{primary}}}",
                    w.text,
                    accent = style.accent_color,
                    primary = style.primary_color,
                )
            } else {
                w.text.clone()
            }
        })
        .collect();
    format!("Dialogue: 0,{start},{end},Default,,0,0,0,,{}\n", text.join(" "))
}

/// Renders a phrase as a sequence of Dialogue lines — one per word, each
/// statically highlighting that word. Timed so exactly one event is on screen
/// at any moment the phrase is visible, and the accent visibly moves.
///
/// Using per-word Dialogue events (rather than `\t` animations inside a single
/// event) avoids a libass quirk where `\t` transforms cascade forward across
/// spans in the same line, producing an "all words light up then peel off"
/// rendering instead of a classic moving highlight.
///
/// If `style.first_word_lead_in_ms > 0`, an initial lead-in event (phrase text
/// with no word accented) is emitted so the viewer's eye can find the new
/// caption before any word lights up.
pub fn phrase_to_ass_events(phrase: &Phrase, style: &AssStyle) -> String {
    if phrase.words.is_empty() {
        return String::new();
    }
    let phrase_start_ms = phrase.words.first().unwrap().start_ms;
    let phrase_end_ms = phrase.words.last().unwrap().end_ms;
    let lead_in_ms = style.first_word_lead_in_ms as i64;

    let mut out = String::new();

    if lead_in_ms > 0 {
        out.push_str(&build_dialogue_line(
            phrase_start_ms,
            phrase_start_ms + lead_in_ms,
            &phrase.words,
            None,
            style,
        ));
    }

    for (i, word) in phrase.words.iter().enumerate() {
        let start = if i == 0 {
            phrase_start_ms + lead_in_ms
        } else {
            word.start_ms
        };
        let end = phrase
            .words
            .get(i + 1)
            .map(|w| w.start_ms)
            .unwrap_or(phrase_end_ms);
        if start >= end {
            continue;
        }
        out.push_str(&build_dialogue_line(
            start,
            end,
            &phrase.words,
            Some(i),
            style,
        ));
    }
    out
}

/// Builds the ASS header block. Pure function. `play_res_x`/`play_res_y` must
/// match the burn output dimensions so libass scales font/margins correctly.
pub fn build_ass_header(style: &AssStyle, play_res_x: u32, play_res_y: u32) -> String {
    format!(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: {px}\n\
         PlayResY: {py}\n\
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
        px = play_res_x,
        py = play_res_y,
        font = style.font_name,
        size = style.font_size,
        primary = style.primary_color,
        outline = style.outline_color,
        outline_w = style.outline_width,
        margin_v = style.margin_v,
    )
}

/// Converts a WhisperOutput to a complete ASS file string. Fully pure.
pub fn generate_ass(
    output: &WhisperOutput,
    style: &AssStyle,
    play_res_x: u32,
    play_res_y: u32,
) -> String {
    let words = flatten_words(output);
    let phrases = words_to_phrases(&words, style.words_per_phrase);
    let mut ass = build_ass_header(style, play_res_x, play_res_y);
    for phrase in &phrases {
        ass.push_str(&phrase_to_ass_events(phrase, style));
    }
    ass
}

/// Writes `content` to `<folder>/<stem>_<slug>.ass`. The folder must already exist.
pub fn write_ass_file(
    folder: &Path,
    stem: &str,
    slug: &str,
    content: &str,
) -> Result<PathBuf, StageError> {
    let out = folder.join(format!("{stem}_{slug}.ass"));
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

    // --- phrase_to_ass_events ---

    fn dialogue_count(output: &str) -> usize {
        output.matches("Dialogue:").count()
    }

    #[test]
    fn phrase_emits_one_event_per_word_plus_lead_in() {
        let tokens: Vec<_> = (0..5).map(|i| tok(" w", i * 500, (i + 1) * 500, 0.9)).collect();
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle::default();  // lead_in = 100
        let out = phrase_to_ass_events(&phrases[0], &style);
        // 1 lead-in event + 5 word events
        assert_eq!(dialogue_count(&out), 6);
    }

    #[test]
    fn no_lead_in_skips_plain_event() {
        let tokens: Vec<_> = (0..5).map(|i| tok(" w", i * 500, (i + 1) * 500, 0.9)).collect();
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle { first_word_lead_in_ms: 0, ..AssStyle::default() };
        let out = phrase_to_ass_events(&phrases[0], &style);
        // 0 lead-in + 5 word events
        assert_eq!(dialogue_count(&out), 5);
    }

    #[test]
    fn each_word_event_has_exactly_one_accent_wrap() {
        let tokens: Vec<_> = (0..3).map(|i| tok(" w", i * 500, (i + 1) * 500, 0.9)).collect();
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle { first_word_lead_in_ms: 0, ..AssStyle::default() };
        let out = phrase_to_ass_events(&phrases[0], &style);
        // Each word event has one open accent and one close primary
        assert_eq!(out.matches(&format!("{{\\c{}}}", style.accent_color)).count(), 3);
        assert_eq!(out.matches(&format!("{{\\c{}}}", style.primary_color)).count(), 3);
    }

    #[test]
    fn lead_in_event_has_no_accent_wrap() {
        let tokens = vec![
            tok(" alpha", 0, 500, 0.9),
            tok(" beta",  500, 1000, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle::default();  // lead_in = 100
        let out = phrase_to_ass_events(&phrases[0], &style);
        // Lead-in event is the first Dialogue line and has no accent wrap
        let first_line = out.lines().next().unwrap();
        assert!(first_line.starts_with("Dialogue:"));
        assert!(!first_line.contains(&format!("{{\\c{}}}", style.accent_color)));
        // It still contains the phrase text in full
        assert!(first_line.contains("alpha"));
        assert!(first_line.contains("beta"));
    }

    #[test]
    fn accent_moves_across_events() {
        // For a 3-word phrase with lead-in disabled, event N should accent word N.
        let tokens = vec![
            tok(" ALPHA", 0,   400, 0.9),
            tok(" BETA",  400, 800, 0.9),
            tok(" GAMMA", 800, 1200, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle { first_word_lead_in_ms: 0, ..AssStyle::default() };
        let out = phrase_to_ass_events(&phrases[0], &style);
        let lines: Vec<&str> = out.lines().filter(|l| l.starts_with("Dialogue:")).collect();
        assert_eq!(lines.len(), 3);
        // Event 0 wraps ALPHA only
        assert!(lines[0].contains(&format!("{{\\c{}}}ALPHA", style.accent_color)));
        assert!(!lines[0].contains(&format!("{{\\c{}}}BETA", style.accent_color)));
        // Event 1 wraps BETA only
        assert!(lines[1].contains(&format!("{{\\c{}}}BETA", style.accent_color)));
        assert!(!lines[1].contains(&format!("{{\\c{}}}ALPHA", style.accent_color)));
        // Event 2 wraps GAMMA only
        assert!(lines[2].contains(&format!("{{\\c{}}}GAMMA", style.accent_color)));
    }

    #[test]
    fn word_event_end_equals_next_word_start() {
        // Event i ends at word[i+1].start_ms; last event ends at phrase_last_word.end_ms.
        let tokens = vec![
            tok(" a", 0,    300,  0.9),
            tok(" b", 500,  900,  0.9),  // gap between a.end=300 and b.start=500
            tok(" c", 1100, 1500, 0.9),
        ];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle { first_word_lead_in_ms: 0, ..AssStyle::default() };
        let out = phrase_to_ass_events(&phrases[0], &style);
        let lines: Vec<&str> = out.lines().filter(|l| l.starts_with("Dialogue:")).collect();
        // Event 0: [0, 500] — ends at word b's start, not word a's end
        assert!(lines[0].contains(&format!(",{},", ms_to_ass(500))));
        // Event 1: [500, 1100] — ends at word c's start
        assert!(lines[1].contains(&format!(",{},", ms_to_ass(1100))));
        // Event 2 (last): [1100, 1500] — ends at phrase_last_word.end
        assert!(lines[2].contains(&format!(",{},", ms_to_ass(1500))));
    }

    #[test]
    fn events_use_style_colors_not_hardcoded() {
        let tokens = vec![tok(" word", 0, 500, 0.9)];
        let words = flatten_words(&make_output(tokens));
        let phrases = words_to_phrases(&words, 5);
        let style = AssStyle {
            accent_color: "&H0000AAAA".to_string(),
            primary_color: "&H00BBBBBB".to_string(),
            first_word_lead_in_ms: 0,
            ..AssStyle::default()
        };
        let out = phrase_to_ass_events(&phrases[0], &style);
        assert!(out.contains("&H0000AAAA"));
        assert!(out.contains("&H00BBBBBB"));
        // Default yellow / white must NOT leak through when style overrides them
        assert!(!out.contains("&H0000FFFF"));
        assert!(!out.contains("&H00FFFFFF"));
    }

    // --- build_ass_header ---

    #[test]
    fn header_contains_script_info() {
        let style = AssStyle::default();
        let header = build_ass_header(&style, 1920, 1080);
        assert!(header.contains("[Script Info]"));
        assert!(header.contains("[V4+ Styles]"));
        assert!(header.contains("[Events]"));
    }

    #[test]
    fn header_contains_style_values() {
        let style = AssStyle::default();
        let header = build_ass_header(&style, 1920, 1080);
        assert!(header.contains("Arial"));
        assert!(header.contains("72"));
    }

    #[test]
    fn header_uses_provided_play_res() {
        let style = AssStyle::default();
        let header = build_ass_header(&style, 1080, 1920);
        assert!(header.contains("PlayResX: 1080"));
        assert!(header.contains("PlayResY: 1920"));
    }

    // --- generate_ass (end-to-end) ---

    #[test]
    fn generate_ass_correct_dialogue_count() {
        // 6 words, target 5 → 2 phrases.
        // With default lead-in (100ms): phrase 1 = 5 words → 1 lead-in + 5 word events = 6.
        // Phrase 2 = 1 word → 1 lead-in + 1 word event = 2.
        // Total = 8 Dialogue lines.
        let tokens: Vec<_> = (0..6)
            .map(|i| tok(" word", i * 500, (i + 1) * 500, 0.9))
            .collect();
        let output = make_output(tokens);
        let style = AssStyle::default();
        let ass = generate_ass(&output, &style, 1920, 1080);
        assert_eq!(ass.matches("Dialogue:").count(), 8);
    }

    #[test]
    fn generate_ass_well_formed() {
        let tokens: Vec<_> = (0..5)
            .map(|i| tok(" hello", i * 500, (i + 1) * 500, 0.9))
            .collect();
        let output = make_output(tokens);
        let style = AssStyle::default();
        let ass = generate_ass(&output, &style, 1920, 1080);
        assert!(ass.starts_with("[Script Info]"));
        assert!(ass.contains("Dialogue:"));
    }
}
