pub mod bottom_panel_render;
pub mod command_line_ui;
pub mod editor_view;
pub mod highlight_render;
pub mod popup;
pub mod sidebar_render;
pub mod statusline;
pub mod tabline;

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::App;
use crate::input::EditorMode;
use crate::layout::View;

/// Adjust `view.scroll.0` so the cursor row stays within the visible
/// window, with a small scroll-off margin top and bottom.
pub fn clamp_scroll_to_cursor(view: &mut View, area_height: usize) {
    if area_height == 0 {
        return;
    }
    let scroll_off = 3.min(area_height / 2);
    let cursor_row = view.cursor.0;
    let top = view.scroll.0;
    // Cursor above viewport (with margin)
    if cursor_row < top + scroll_off {
        view.scroll.0 = cursor_row.saturating_sub(scroll_off);
    }
    // Cursor below viewport (with margin)
    let bottom = top + area_height;
    if cursor_row + scroll_off + 1 > bottom {
        view.scroll.0 = cursor_row + scroll_off + 1 - area_height;
    }
}

/// Top-level renderer. Lays out tabline / sidebars / editor / statusline / command-line / popups.
pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let size = frame.area();
    if size.width == 0 || size.height == 0 {
        return;
    }

    let show_tabline = app.config.ui.tabline && !app.layout.tabs.is_empty();
    let show_status = app.config.ui.statusline;
    let in_cmdline = matches!(app.mode, EditorMode::Command | EditorMode::Search)
        || app.status_message.is_some();

    // Reserve top row for tabline.
    let mut y = size.y;
    let mut h = size.height;

    let tab_rect = if show_tabline && h > 0 {
        let r = Rect {
            x: size.x,
            y,
            width: size.width,
            height: 1,
        };
        y += 1;
        h = h.saturating_sub(1);
        Some(r)
    } else {
        None
    };

    // Reserve bottom row(s) for statusline + command-line.
    let cmdline_rect = if in_cmdline && h > 0 {
        let r = Rect {
            x: size.x,
            y: y + h - 1,
            width: size.width,
            height: 1,
        };
        h = h.saturating_sub(1);
        Some(r)
    } else {
        None
    };

    let status_rect = if show_status && h > 0 {
        let r = Rect {
            x: size.x,
            y: y + h - 1,
            width: size.width,
            height: 1,
        };
        h = h.saturating_sub(1);
        Some(r)
    } else {
        None
    };

    // Bottom panel: carve rows from the bottom of the remaining middle band
    // BEFORE sidebars carve columns, so the sidebars stop above the panel.
    let bottom_panel_rect = if app.layout.bottom_panel.open && h > 0 {
        let panel_h = app.layout.bottom_panel.height.min(h.saturating_sub(1)).max(3);
        let r = Rect {
            x: size.x,
            y: y + h - panel_h,
            width: size.width,
            height: panel_h,
        };
        h = h.saturating_sub(panel_h);
        Some(r)
    } else {
        None
    };

    // Sidebars consume columns from the middle band.
    let middle_y = y;
    let middle_h = h;
    let mut middle_x = size.x;
    let mut middle_w = size.width;

    let left_w = if app.layout.left_sidebar.open {
        app.layout.left_sidebar.width.min(middle_w)
    } else {
        0
    };
    let right_w = if app.layout.right_sidebar.open {
        app.layout
            .right_sidebar
            .width
            .min(middle_w.saturating_sub(left_w))
    } else {
        0
    };

    let left_rect = if left_w > 0 {
        let r = Rect {
            x: middle_x,
            y: middle_y,
            width: left_w,
            height: middle_h,
        };
        middle_x += left_w;
        middle_w -= left_w;
        Some(r)
    } else {
        None
    };

    let right_rect = if right_w > 0 {
        let r = Rect {
            x: middle_x + middle_w - right_w,
            y: middle_y,
            width: right_w,
            height: middle_h,
        };
        middle_w -= right_w;
        Some(r)
    } else {
        None
    };

    let editor_rect = Rect {
        x: middle_x,
        y: middle_y,
        width: middle_w,
        height: middle_h,
    };

    // Tabline
    if let Some(r) = tab_rect {
        tabline::render(frame, app, r);
    } else {
        app.last_tab_rects.clear();
    }

    // Sidebars
    if let Some(r) = left_rect {
        sidebar_render::render(frame, &app.layout.left_sidebar, r, "Explorer");
    }
    if let Some(r) = right_rect {
        sidebar_render::render(frame, &app.layout.right_sidebar, r, "Outline");
    }

    // Editor: layout the active tab's split tree.
    app.last_editor_rect = editor_rect;
    app.last_editor_view_rects.clear();
    if editor_rect.width > 0 && editor_rect.height > 0 {
        // First pass: compute rects and clamp each view's scroll so the
        // cursor stays inside the visible area.
        if let Some(tab) = app.layout.active_tab_mut() {
            let leaves = tab.root.layout_rects(editor_rect);
            for (vid, rect) in &leaves {
                if let Some(view) = tab.root.find_mut(*vid) {
                    clamp_scroll_to_cursor(view, rect.height as usize);
                }
            }
        }
        // Snapshot the layout so we can free the immutable borrow on
        // `app.layout` before calling `editor_view::render`, which needs
        // `&mut App` for the highlight engine.
        let snapshot: Option<(Vec<(crate::app::ViewId, Rect)>, crate::app::ViewId, Vec<(crate::app::ViewId, crate::layout::View)>)> = app
            .layout
            .active_tab()
            .map(|tab| {
                let leaves = tab.root.layout_rects(editor_rect);
                let views: Vec<_> = leaves
                    .iter()
                    .filter_map(|(vid, _)| tab.root.find(*vid).map(|v| (*vid, v.clone())))
                    .collect();
                (leaves, tab.active_view, views)
            });
        if let Some((leaves, active_view, views)) = snapshot {
            for (vid, view) in &views {
                if let Some((_, rect)) = leaves.iter().find(|(v, _)| v == vid) {
                    let is_active = *vid == active_view;
                    editor_view::render(frame, app, view, *rect, is_active);
                }
            }
            app.last_editor_view_rects = leaves;
        }
    }
    app.last_left_sidebar_rect = left_rect.unwrap_or_default();

    // Bottom panel — refresh its problems pane from the active buffer's
    // diagnostics, then render.
    if let Some(r) = bottom_panel_rect {
        refresh_bottom_panel(app);
        bottom_panel_render::render(frame, &app.layout.bottom_panel, r, app.panel_focused);
        app.last_bottom_panel_rect = r;
    } else {
        app.last_bottom_panel_rect = ratatui::layout::Rect::default();
    }

    // Statusline (overlaid by command line when in command/search mode).
    if let Some(r) = status_rect {
        statusline::render(frame, app, r);
    }
    if let Some(r) = cmdline_rect {
        command_line_ui::render(frame, app, r);
    }

    // Popups (drawn last so they're on top).
    popup::render(frame, app, editor_rect);
    popup::render_leader_popup(frame, app, editor_rect);
    render_diagnostic_tooltip(frame, app);
}

/// Sync the bottom panel's problems pane with the current buffer's
/// LSP diagnostics. Called every frame the panel is open.
fn refresh_bottom_panel(app: &mut App) {
    use crate::panes::lsp_problems::LspProblemsPane;
    // Snapshot diagnostics first to release the lock before we mutate panes.
    let diags_snapshot: Option<Vec<lsp_types::Diagnostic>> = (|| {
        let tab = app.layout.active_tab()?;
        let view = tab.root.find(tab.active_view)?;
        let buf = app.buffers.get(view.buffer_id.0 as usize)?;
        let uri = buf.lsp_uri.as_ref()?;
        Some(app.lsp.diagnostics.lock().get(uri).to_vec())
    })();
    let _ = LspProblemsPane::default; // keep type referenced for clarity
    if let Some(diags) = diags_snapshot {
        for pane in app.layout.bottom_panel.panes.iter_mut() {
            pane.refresh_diagnostics(&diags);
        }
    }
}

/// Minimal CommonMark-ish renderer geared at LSP hover responses
/// (rust-analyzer in particular). Handles:
///   - fenced code blocks ```lang ... ``` (rendered with code styling,
///     fence markers stripped, blank-line padding around them collapsed)
///   - ATX headings (`#`, `##`, `###` …) → bold
///   - horizontal rules (`---`, `***`, `___`) → thin line
///   - inline code `…` → cyan
///   - bold `**…**` / `__…__` → bold modifier
///   - italic `*…*` / `_…_` → italic modifier
///   - blockquote `> …` → dimmed
/// Anything not matched falls through as plain text.
fn render_markdown(text: &str) -> Vec<ratatui::text::Line<'static>> {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};

    let code_style = Style::default().fg(Color::LightCyan);
    let heading_style = Style::default()
        .fg(Color::LightYellow)
        .add_modifier(Modifier::BOLD);
    let rule_style = Style::default().fg(Color::DarkGray);

    let mut out: Vec<Line> = Vec::new();
    let mut in_code_block = false;
    let mut prev_blank = true;

    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');

        // Fenced code block toggles.
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            // Drop the fence line entirely.
            continue;
        }

        if in_code_block {
            out.push(Line::from(Span::styled(line.to_string(), code_style)));
            prev_blank = false;
            continue;
        }

        // Collapse runs of blank lines to one.
        if line.trim().is_empty() {
            if !prev_blank {
                out.push(Line::from(""));
                prev_blank = true;
            }
            continue;
        }
        prev_blank = false;

        // Horizontal rule.
        let t = line.trim();
        if (t.starts_with("---") || t.starts_with("***") || t.starts_with("___"))
            && t.chars().all(|c| c == '-' || c == '*' || c == '_')
        {
            out.push(Line::from(Span::styled("─".repeat(20), rule_style)));
            continue;
        }

        // ATX heading.
        if let Some(rest) = t.strip_prefix("######") {
            out.push(Line::from(Span::styled(rest.trim().to_string(), heading_style)));
            continue;
        }
        for prefix in ["#####", "####", "###", "##", "#"] {
            if let Some(rest) = t.strip_prefix(prefix) {
                if rest.starts_with(' ') || rest.is_empty() {
                    out.push(Line::from(Span::styled(
                        rest.trim().to_string(),
                        heading_style,
                    )));
                    break;
                }
            }
        }
        if t.starts_with('#') {
            // We pushed inside the loop above; skip falling through.
            continue;
        }

        // Blockquote.
        if let Some(rest) = t.strip_prefix("> ") {
            out.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
            continue;
        }

        // Inline span parsing on the (still indented) line.
        out.push(Line::from(parse_inline(line)));
    }

    // Trim leading/trailing blank lines.
    while out.first().map(|l| line_is_blank(l)).unwrap_or(false) {
        out.remove(0);
    }
    while out.last().map(|l| line_is_blank(l)).unwrap_or(false) {
        out.pop();
    }
    out
}

fn line_is_blank(l: &ratatui::text::Line<'_>) -> bool {
    l.spans.iter().all(|s| s.content.trim().is_empty())
}

/// Parse inline emphasis / code into styled spans for one line.
fn parse_inline(line: &str) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::Span;

    let code_style = Style::default().fg(Color::LightCyan);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let italic = Style::default().add_modifier(Modifier::ITALIC);

    let bytes = line.as_bytes();
    let mut out: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let mut i = 0;

    let flush = |out: &mut Vec<Span>, buf: &mut String| {
        if !buf.is_empty() {
            out.push(Span::raw(std::mem::take(buf)));
        }
    };

    while i < bytes.len() {
        let c = bytes[i];
        // Inline code: `…`
        if c == b'`' {
            if let Some(end) = line[i + 1..].find('`') {
                flush(&mut out, &mut buf);
                let code = &line[i + 1..i + 1 + end];
                out.push(Span::styled(code.to_string(), code_style));
                i += 1 + end + 1;
                continue;
            }
        }
        // Bold: **…** or __…__
        if i + 1 < bytes.len() && (c == b'*' || c == b'_') && bytes[i + 1] == c {
            let marker = &line[i..i + 2];
            if let Some(end) = line[i + 2..].find(marker) {
                flush(&mut out, &mut buf);
                let inner = &line[i + 2..i + 2 + end];
                out.push(Span::styled(inner.to_string(), bold));
                i += 2 + end + 2;
                continue;
            }
        }
        // Italic: *…* or _…_
        if (c == b'*' || c == b'_')
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
        {
            let marker = c as char;
            if let Some(end) = line[i + 1..].find(marker) {
                let inner = &line[i + 1..i + 1 + end];
                if !inner.is_empty() && !inner.starts_with(' ') && !inner.ends_with(' ') {
                    flush(&mut out, &mut buf);
                    out.push(Span::styled(inner.to_string(), italic));
                    i += 1 + end + 1;
                    continue;
                }
            }
        }
        // Markdown link [text](url) → render just `text`
        if c == b'[' {
            if let Some(close) = line[i + 1..].find(']') {
                let after = i + 1 + close + 1;
                if after < bytes.len() && bytes[after] == b'(' {
                    if let Some(paren) = line[after + 1..].find(')') {
                        flush(&mut out, &mut buf);
                        let text = &line[i + 1..i + 1 + close];
                        out.push(Span::styled(
                            text.to_string(),
                            Style::default().fg(Color::LightBlue),
                        ));
                        i = after + 1 + paren + 1;
                        continue;
                    }
                }
            }
        }

        // Default: append the next UTF-8 char.
        let ch_len = utf8_len(c);
        buf.push_str(&line[i..i + ch_len]);
        i += ch_len;
    }
    flush(&mut out, &mut buf);
    if out.is_empty() {
        out.push(Span::raw(String::new()));
    }
    out
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b < 0xC0 {
        1 // continuation byte (shouldn't happen at boundary)
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

/// Render the on-demand info popup populated by `lsp.hover`.
fn render_diagnostic_tooltip(frame: &mut Frame<'_>, app: &App) {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let Some(popup) = &app.hover_popup else {
        return;
    };
    let Some(tab) = app.layout.active_tab() else {
        return;
    };
    let Some(view) = tab.root.find(tab.active_view) else {
        return;
    };
    let Some((_, view_rect)) = app
        .last_editor_view_rects
        .iter()
        .find(|(v, _)| *v == tab.active_view)
    else {
        return;
    };

    let cursor_row = popup.anchor_row;
    let cursor_col = popup.anchor_col;

    // Build wrapped text lines: status line, then diagnostics, then hover.
    let mut lines: Vec<Line> = Vec::new();
    if popup.loading {
        lines.push(Line::from(Span::styled(
            "loading hover…",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
    }
    for d in &popup.diagnostics {
        let (label, color) = match d.severity {
            Some(lsp_types::DiagnosticSeverity::ERROR) => ("error", Color::Red),
            Some(lsp_types::DiagnosticSeverity::WARNING) => ("warn", Color::Yellow),
            Some(lsp_types::DiagnosticSeverity::INFORMATION) => ("info", Color::Cyan),
            Some(lsp_types::DiagnosticSeverity::HINT) => ("hint", Color::Gray),
            _ => ("diag", Color::White),
        };
        for (i, msg_line) in d.message.lines().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{label}: "),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(msg_line.to_string()),
                ]));
            } else {
                lines.push(Line::from(Span::raw(format!("  {msg_line}"))));
            }
        }
    }
    if let Some(text) = &popup.hover_text {
        if !popup.diagnostics.is_empty() || popup.loading {
            lines.push(Line::from(Span::styled(
                "─",
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.extend(render_markdown(text));
    }
    if lines.is_empty() {
        return;
    }

    // Compute popup size.
    let max_w = view_rect.width.saturating_sub(2).max(20);
    let content_w = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.chars().count()).sum::<usize>())
        .max()
        .unwrap_or(0) as u16;
    let popup_w = (content_w + 2).min(max_w).max(10);
    // Wrap-aware height: rough ceil(content_w / inner_w).
    let inner_w = popup_w.saturating_sub(2).max(1) as usize;
    let mut popup_h: u16 = 0;
    for l in &lines {
        let w: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
        let rows = (w.max(1) + inner_w - 1) / inner_w;
        popup_h = popup_h.saturating_add(rows as u16);
    }
    popup_h = (popup_h + 2).min(view_rect.height.saturating_sub(1)).max(3);

    // Cursor screen position.
    let scroll_row = view.scroll.0;
    if cursor_row < scroll_row {
        return;
    }
    let cursor_screen_y = view_rect.y + (cursor_row - scroll_row) as u16;

    // Prefer placing the popup BELOW the cursor row; fall back to above.
    let space_below = view_rect
        .y
        .saturating_add(view_rect.height)
        .saturating_sub(cursor_screen_y + 1);
    let space_above = cursor_screen_y.saturating_sub(view_rect.y);
    let (popup_y, popup_h) = if space_below >= popup_h {
        (cursor_screen_y + 1, popup_h)
    } else if space_above >= popup_h {
        (cursor_screen_y.saturating_sub(popup_h), popup_h)
    } else if space_below >= space_above {
        (cursor_screen_y + 1, space_below.max(1))
    } else {
        let h = space_above.max(1);
        (cursor_screen_y.saturating_sub(h), h)
    };

    // Horizontal placement: anchor near the cursor column but keep inside
    // the view rect.
    let scroll_col = view.scroll.1;
    let cursor_screen_x = view_rect.x
        + (cursor_col.saturating_sub(scroll_col)) as u16;
    let max_x = view_rect.x + view_rect.width.saturating_sub(popup_w);
    let popup_x = cursor_screen_x.min(max_x).max(view_rect.x);

    let area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_w,
        height: popup_h,
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(Color::Black));
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn renders_empty_with_statusline() {
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = App::new();
        term.draw(|f| render(f, &mut app)).unwrap();
        // Last row should contain the NORMAL mode badge.
        let buf = term.backend().buffer().clone();
        let mut bottom = String::new();
        for x in 0..buf.area.width {
            bottom.push_str(buf[(x, buf.area.height - 1)].symbol());
        }
        assert!(
            bottom.contains("NORMAL"),
            "expected NORMAL in statusline, got {bottom:?}"
        );
    }

    #[test]
    fn screen_to_buffer_accounts_for_scroll_and_gutter() {
        use crate::app::{BufferId, ViewId};
        use crate::layout::View;
        let mut v = View::new(ViewId(1), BufferId(0));
        v.scroll = (10, 5);
        let (row, col) = v.screen_to_buffer(2, 8, 4);
        assert_eq!(row, 12);
        assert_eq!(col, 9);
    }
}
