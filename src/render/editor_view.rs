use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::layout::View;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumberStyle {
    None,
    Absolute,
    Relative,
}

pub fn line_number_style(app: &App) -> LineNumberStyle {
    let show = app.config.editor.line_numbers.unwrap_or(true);
    let rel = app.config.editor.relative_line_numbers.unwrap_or(false);
    match (show, rel) {
        (false, _) => LineNumberStyle::None,
        (true, false) => LineNumberStyle::Absolute,
        (true, true) => LineNumberStyle::Relative,
    }
}

/// Width of the gutter (line-number column) for a given total line count.
pub fn gutter_width(style: LineNumberStyle, total_lines: usize) -> u16 {
    match style {
        LineNumberStyle::None => 0,
        _ => {
            let digits = total_lines.max(1).to_string().len() as u16;
            digits.max(3) + 1
        }
    }
}

pub fn render(frame: &mut Frame<'_>, app: &App, view: &View, area: Rect, is_active: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let buf = app.buffers.iter().find(|b| b.id == view.buffer_id);
    let total_lines = buf.map(|b| b.rope.len_lines()).unwrap_or(0);
    let ln_style = line_number_style(app);
    let gw = gutter_width(ln_style, total_lines.max(1));

    let cursor_row = view.cursor.0;
    let cursor_col = view.cursor.1;
    let scroll_row = view.scroll.0;
    let scroll_col = view.scroll.1;

    let mut lines: Vec<Line> = Vec::with_capacity(area.height as usize);
    for screen_row in 0..area.height as usize {
        let buf_row = scroll_row + screen_row;
        let mut spans: Vec<Span> = Vec::new();

        if gw > 0 {
            let label = match ln_style {
                LineNumberStyle::Absolute => {
                    if buf_row < total_lines {
                        format!("{:>width$} ", buf_row + 1, width = (gw - 1) as usize)
                    } else {
                        format!("{:>width$} ", "~", width = (gw - 1) as usize)
                    }
                }
                LineNumberStyle::Relative => {
                    if buf_row < total_lines {
                        let n = if buf_row == cursor_row {
                            buf_row + 1
                        } else {
                            (buf_row as isize - cursor_row as isize).unsigned_abs()
                        };
                        format!("{:>width$} ", n, width = (gw - 1) as usize)
                    } else {
                        format!("{:>width$} ", "~", width = (gw - 1) as usize)
                    }
                }
                LineNumberStyle::None => String::new(),
            };
            spans.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
        }

        let text_w = (area.width.saturating_sub(gw)) as usize;
        let line_text: String = if let Some(b) = buf {
            if buf_row < total_lines {
                let line = b.rope.line(buf_row);
                let s: String = line.chars().collect();
                let s = s.trim_end_matches('\n').to_string();
                let chars: Vec<char> = s.chars().collect();
                if scroll_col >= chars.len() {
                    String::new()
                } else {
                    chars[scroll_col..].iter().take(text_w).collect()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Render text, painting the cursor cell if it's on this row and active.
        if is_active && buf_row == cursor_row && cursor_col >= scroll_col {
            let rel_col = cursor_col - scroll_col;
            let chars: Vec<char> = line_text.chars().collect();
            let before: String = chars.iter().take(rel_col).collect();
            let cursor_char = chars.get(rel_col).copied().unwrap_or(' ');
            let after: String = chars.iter().skip(rel_col + 1).collect();
            if !before.is_empty() {
                spans.push(Span::raw(before));
            }
            spans.push(Span::styled(
                cursor_char.to_string(),
                Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::REVERSED),
            ));
            if !after.is_empty() {
                spans.push(Span::raw(after));
            }
        } else {
            spans.push(Span::raw(line_text));
        }

        lines.push(Line::from(spans));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}
