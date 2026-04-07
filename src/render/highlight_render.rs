//! Convert `HighlightSpan`s into per-line ratatui spans for the editor view.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::highlight::HighlightSpan;

/// A byte-ranged diagnostic overlay: underline + color.
#[derive(Debug, Clone, Copy)]
pub struct DiagSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub color: Color,
}

/// Build the styled spans for the visible portion of one line.
///
/// * `line_text` — the raw line text (no trailing newline).
/// * `line_start_byte` — the byte offset of this line within the full buffer.
/// * `scroll_col` — number of leading chars to skip (horizontal scroll).
/// * `text_w` — max chars to emit after skipping.
/// * `spans` — all highlight spans for the buffer, byte-ranged against the rope.
pub fn style_line(
    line_text: &str,
    line_start_byte: usize,
    scroll_col: usize,
    text_w: usize,
    spans: &[HighlightSpan],
    diags: &[DiagSpan],
) -> Vec<Span<'static>> {
    if line_text.is_empty() || text_w == 0 {
        return Vec::new();
    }
    let line_end_byte = line_start_byte + line_text.len();

    // Clip to overlapping spans, expressed in line-local byte offsets.
    // Sort by (start, end) so the first match wins deterministically.
    let mut clipped: Vec<(usize, usize, Style)> = spans
        .iter()
        .filter(|s| s.end_byte > line_start_byte && s.start_byte < line_end_byte)
        .map(|s| {
            let start = s.start_byte.saturating_sub(line_start_byte);
            let end = (s.end_byte - line_start_byte).min(line_text.len());
            (start, end, s.style)
        })
        .collect();
    clipped.sort_by_key(|(s, e, _)| (*s, *e));

    let style_at = |byte: usize| -> Style {
        // First (smallest-start) span that covers `byte`.
        for (s, e, st) in &clipped {
            if *s > byte {
                break;
            }
            if byte < *e {
                return *st;
            }
        }
        Style::default()
    };

    // Clip diagnostics to this line, in line-local byte offsets.
    // Include ranges that end exactly at or past the line end — rust-analyzer
    // often reports zero-width syntax errors at column = line_len, and we want
    // those to land on the final character of the line rather than vanish.
    let clipped_diags: Vec<(usize, usize, Color)> = diags
        .iter()
        .filter(|d| d.end_byte >= line_start_byte && d.start_byte <= line_end_byte)
        .map(|d| {
            let mut start = d.start_byte.saturating_sub(line_start_byte);
            let mut end = (d.end_byte - line_start_byte).min(line_text.len());
            // Zero-width or past-end: underline the last char of the line
            // (or the first char if the line is empty — clamped below).
            if end <= start {
                if line_text.is_empty() {
                    start = 0;
                    end = 0;
                } else {
                    // Snap to the word containing (or just before) `start`.
                    let bytes = line_text.as_bytes();
                    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
                    // If we're past the end of the line, step back one byte
                    // so we're sitting on a real character.
                    let probe = start.min(line_text.len().saturating_sub(1));
                    // Find a word char at or before `probe`.
                    let mut ws = probe;
                    while ws > 0 && !is_word(bytes[ws]) {
                        ws -= 1;
                    }
                    if is_word(bytes[ws]) {
                        // Walk left to word start.
                        while ws > 0 && is_word(bytes[ws - 1]) {
                            ws -= 1;
                        }
                        // Walk right to word end.
                        let mut we = ws;
                        while we < bytes.len() && is_word(bytes[we]) {
                            we += 1;
                        }
                        start = ws;
                        end = we;
                    } else {
                        start = probe;
                        end = probe + 1;
                    }
                }
            }
            (start, end, d.color)
        })
        .collect();
    let diag_at = |byte: usize| -> Option<Color> {
        clipped_diags
            .iter()
            .find(|(s, e, _)| byte >= *s && byte < *e)
            .map(|(_, _, c)| *c)
    };

    // Walk chars, tracking running char index (for scroll_col/text_w) and
    // byte index (for style lookup). Coalesce contiguous same-style chars.
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_style: Option<Style> = None;
    let mut char_idx = 0usize;
    let mut emitted = 0usize;
    let mut byte_idx = 0usize;

    for ch in line_text.chars() {
        let ch_byte = byte_idx;
        let ch_len = ch.len_utf8();
        byte_idx += ch_len;

        if char_idx < scroll_col {
            char_idx += 1;
            continue;
        }
        if emitted >= text_w {
            break;
        }
        char_idx += 1;
        emitted += 1;

        let mut st = style_at(ch_byte);
        if let Some(c) = diag_at(ch_byte) {
            st = st.fg(c).add_modifier(Modifier::UNDERLINED);
        }
        if current_style != Some(st) {
            if !current_text.is_empty() {
                out.push(Span::styled(
                    std::mem::take(&mut current_text),
                    current_style.unwrap_or_default(),
                ));
            }
            current_style = Some(st);
        }
        current_text.push(ch);
    }
    if !current_text.is_empty() {
        out.push(Span::styled(current_text, current_style.unwrap_or_default()));
    }
    out
}
