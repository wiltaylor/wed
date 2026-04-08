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

pub fn render(
    frame: &mut Frame<'_>,
    app: &mut App,
    view: &View,
    area: Rect,
    is_active: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let buffer_idx = view.buffer_id.0 as usize;

    // Compute highlight spans up front, using a split-borrow so we can
    // mutably borrow `highlight` and `buffers` simultaneously.
    let highlight_spans: Vec<crate::highlight::HighlightSpan> = {
        let crate::app::App {
            highlight, buffers, ..
        } = &mut *app;
        if let Some(buf) = buffers.get_mut(buffer_idx) {
            highlight.highlight(buf)
        } else {
            Vec::new()
        }
    };

    let buf = app.buffers.get(buffer_idx);
    let total_lines = buf.map(|b| b.rope.len_lines()).unwrap_or(0);

    // Compute diagnostic byte ranges for the active buffer's URI.
    let diag_spans: Vec<crate::render::highlight_render::DiagSpan> = if let Some(b) = buf {
        if let Some(uri) = &b.lsp_uri {
            use lsp_types::DiagnosticSeverity;
            use ratatui::style::Color;
            let store = app.lsp.diagnostics.lock();
            let diags = store.get(uri);
            let rope = &b.rope;
            // LSP positions use UTF-16 code units for `character`. Convert
            // (line, utf16_col) → absolute byte offset by walking the line's
            // chars and summing utf16 units until we reach the target.
            let resolve = |line: usize, utf16_col: u32| -> Option<usize> {
                if line >= rope.len_lines() {
                    return None;
                }
                let line_byte = rope.line_to_byte(line);
                let line_slice = rope.line(line);
                let mut u16_seen: u32 = 0;
                let mut byte_off: usize = 0;
                for ch in line_slice.chars() {
                    if u16_seen >= utf16_col {
                        break;
                    }
                    u16_seen += ch.len_utf16() as u32;
                    byte_off += ch.len_utf8();
                }
                Some(line_byte + byte_off)
            };
            diags
                .iter()
                .filter_map(|d| {
                    let start_byte = resolve(d.range.start.line as usize, d.range.start.character)?;
                    let end_byte = resolve(d.range.end.line as usize, d.range.end.character)?;
                    let color = match d.severity {
                        Some(DiagnosticSeverity::ERROR) => Color::Red,
                        Some(DiagnosticSeverity::WARNING) => Color::Yellow,
                        Some(DiagnosticSeverity::INFORMATION) => Color::Cyan,
                        _ => Color::Gray,
                    };
                    Some(crate::render::highlight_render::DiagSpan {
                        start_byte,
                        end_byte,
                        color,
                    })
                })
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let ln_style = line_number_style(app);
    let gw = gutter_width(ln_style, total_lines.max(1));

    // Gather DAP state for this buffer: which lines have breakpoints, and
    // (if the debugger is stopped in this file) the 0-based current line.
    let (bp_rows, current_stop_row): (std::collections::HashSet<usize>, Option<usize>) = {
        let canon = buf
            .and_then(|b| b.path.as_ref())
            .and_then(|p| std::fs::canonicalize(p).ok());
        let mut rows = std::collections::HashSet::new();
        if let Some(path) = &canon {
            for (bp_path, list) in &app.dap.breakpoints.files {
                let bp_canon = std::fs::canonicalize(bp_path).unwrap_or_else(|_| bp_path.clone());
                if &bp_canon == path {
                    for b in list {
                        if b.enabled {
                            rows.insert(b.line.saturating_sub(1) as usize);
                        }
                    }
                }
            }
        }
        let stop = app.dap.active().and_then(|s| s.current_line.as_ref()).and_then(|(p, line)| {
            let bc = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            if canon.as_ref() == Some(&bc) {
                Some(line.saturating_sub(1) as usize)
            } else {
                None
            }
        });
        (rows, stop)
    };

    // Annotation rows for this buffer (0-based screen rows).
    let ann_rows: std::collections::HashSet<usize> = {
        let canon = buf
            .and_then(|b| b.path.as_ref())
            .and_then(|p| std::fs::canonicalize(p).ok());
        let mut rows = std::collections::HashSet::new();
        if let Some(path) = &canon {
            for (a_path, list) in &app.annotations.files {
                let a_canon = std::fs::canonicalize(a_path).unwrap_or_else(|_| a_path.clone());
                if &a_canon == path {
                    for a in list {
                        rows.insert(a.line.saturating_sub(1) as usize);
                    }
                }
            }
        }
        rows
    };

    let cursor_row = view.cursor.0;
    let cursor_col = view.cursor.1;
    let scroll_row = view.scroll.0;
    let scroll_col = view.scroll.1;

    let mut lines: Vec<Line> = Vec::with_capacity(area.height as usize);
    for screen_row in 0..area.height as usize {
        let buf_row = scroll_row + screen_row;
        let mut spans: Vec<Span> = Vec::new();

        let has_bp = bp_rows.contains(&buf_row);
        let is_stop = current_stop_row == Some(buf_row);
        let has_ann = ann_rows.contains(&buf_row);
        if gw > 0 {
            let mut label = match ln_style {
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
            // Overlay a leading breakpoint/stop marker in the gutter's first
            // cell. Order of precedence: current-stop arrow beats breakpoint.
            if (has_bp || is_stop || has_ann) && !label.is_empty() {
                // Precedence: stop arrow > breakpoint > annotation.
                let marker = if is_stop {
                    '▶'
                } else if has_bp {
                    '●'
                } else {
                    '◆'
                };
                let mut chars: Vec<char> = label.chars().collect();
                chars[0] = marker;
                label = chars.into_iter().collect();
                let color = if is_stop {
                    Color::Yellow
                } else if has_bp {
                    Color::Red
                } else {
                    Color::Cyan
                };
                // Split the gutter into [marker][rest] so the marker color
                // doesn't bleed into the line number.
                let mut it = label.chars();
                let first: String = it.by_ref().take(1).collect();
                let rest: String = it.collect();
                spans.push(Span::styled(
                    first,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(rest, Style::default().fg(Color::DarkGray)));
            } else {
                spans.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
            }
        }

        let text_w = (area.width.saturating_sub(gw)) as usize;
        let (full_line, line_start_byte): (String, usize) = if let Some(b) = buf {
            if buf_row < total_lines {
                let line = b.rope.line(buf_row);
                let s: String = line.chars().collect();
                let trimmed = s.trim_end_matches('\n').to_string();
                (trimmed, b.rope.line_to_byte(buf_row))
            } else {
                (String::new(), 0)
            }
        } else {
            (String::new(), 0)
        };

        let mut text_spans = crate::render::highlight_render::style_line(
            &full_line,
            line_start_byte,
            scroll_col,
            text_w,
            &highlight_spans,
            &diag_spans,
        );
        if is_stop {
            let bg = Style::default().bg(Color::Rgb(64, 48, 0));
            for s in &mut text_spans {
                s.style = s.style.patch(bg);
            }
        }
        spans.append(&mut text_spans);
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
