use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::layout::View;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumberStyle {
    None,
    Absolute,
    Relative,
}

pub fn line_number_style(app: &App) -> LineNumberStyle {
    let show = app.config.editor.line_numbers;
    let rel = app.config.editor.relative_line_numbers;
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
    let buf = app.buffers.get(view.buffer_id.0 as usize);
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

        spans.push(Span::raw(line_text));
        lines.push(Line::from(spans));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);

    // Position the real terminal cursor for the active view so the user
    // gets the OS-level blinking cursor (shape set in `App::run`).
    if is_active && cursor_row >= scroll_row && cursor_col >= scroll_col {
        let screen_row = (cursor_row - scroll_row) as u16;
        let screen_col = (cursor_col - scroll_col) as u16 + gw;
        if screen_row < area.height && screen_col < area.width {
            frame.set_cursor_position((area.x + screen_col, area.y + screen_row));
        }
    }
}
