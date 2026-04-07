//! Convert `HighlightSpan`s into per-line ratatui spans for the editor view.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::highlight::HighlightSpan;

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

        let st = style_at(ch_byte);
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
