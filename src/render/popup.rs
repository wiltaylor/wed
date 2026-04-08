use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::input::keys::Key;

fn key_label(k: Key) -> String {
    match k {
        Key::Char(' ') => "SPC".into(),
        Key::Char(c) => c.to_string(),
        Key::Enter => "RET".into(),
        Key::Esc => "ESC".into(),
        Key::Tab => "TAB".into(),
        Key::Backspace => "BS".into(),
        Key::Up => "↑".into(),
        Key::Down => "↓".into(),
        Key::Left => "←".into(),
        Key::Right => "→".into(),
        Key::Home => "HOME".into(),
        Key::End => "END".into(),
        Key::PageUp => "PGUP".into(),
        Key::PageDown => "PGDN".into(),
        Key::Delete => "DEL".into(),
        Key::Insert => "INS".into(),
        Key::Ctrl(c) => format!("C-{c}"),
        Key::Alt(c) => format!("M-{c}"),
        Key::F(n) => format!("F{n}"),
        Key::Null => "".into(),
    }
}

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(picker) = app.picker.as_ref() else {
        return;
    };
    if area.width < 10 || area.height < 5 {
        return;
    }
    let w = area.width.saturating_sub(4).min(80);
    let h = area.height.saturating_sub(4).min(20);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let popup_area = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, popup_area);
    let block = Block::default()
        .title(" Files ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    if inner.height < 2 {
        return;
    }
    // Query line at top
    let query_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::raw(app.picker_query.clone()),
    ]);
    let query_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(query_line), query_rect);

    // Match list below
    let list_rect = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height - 1,
    };
    let cap = list_rect.height as usize;
    let start = picker.selected.saturating_sub(cap.saturating_sub(1));
    let lines: Vec<Line> = picker
        .matches
        .iter()
        .enumerate()
        .skip(start)
        .take(cap)
        .map(|(i, (idx, _))| {
            let label = picker
                .items
                .get(*idx)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut style = Style::default().fg(Color::Gray);
            if i == picker.selected {
                style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
            }
            Line::from(Span::styled(label, style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list_rect);
}

/// which-key style leader popup: lists the possible next keys under
/// the current pending leader sequence along with their commands.
pub fn render_leader_popup(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if !app.leader_popup_visible {
        return;
    }
    let Some(seq) = app.leader_seq.as_ref() else {
        return;
    };
    let Some(node) = app.keybindings.leader_trie_at(seq) else {
        return;
    };
    if node.children.is_empty() || area.width < 20 || area.height < 6 {
        return;
    }

    // Collect (key_label, description) pairs.
    let mut entries: Vec<(String, String)> = node
        .children
        .iter()
        .map(|(k, child)| {
            let desc = if let Some(cmd) = &child.command {
                cmd.command.clone()
            } else if !child.children.is_empty() {
                "+prefix".to_string()
            } else {
                String::new()
            };
            (key_label(*k), desc)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Size the popup based on entry count (up to a cap).
    let rows = entries.len().min(area.height.saturating_sub(4) as usize) as u16;
    let desired_h = rows + 2; // borders
    let desired_w = 60u16.min(area.width.saturating_sub(4));
    let h = desired_h.min(area.height);
    let w = desired_w;
    let x = area.x + (area.width - w) / 2;
    // Anchor near the bottom of the editor area, above the statusline.
    let y = area.y + area.height.saturating_sub(h + 1);
    let popup_area = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, popup_area);
    let title = {
        let seq_str: String = seq.iter().copied().map(key_label).collect::<Vec<_>>().join(" ");
        if seq_str.is_empty() {
            " <leader> ".to_string()
        } else {
            format!(" <leader> {seq_str} ")
        }
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let lines: Vec<Line> = entries
        .iter()
        .take(inner.height as usize)
        .map(|(k, desc)| {
            Line::from(vec![
                Span::styled(
                    format!("  {:<6}", k),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("→ {desc}"),
                    Style::default().fg(Color::Gray),
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Render the modal annotation editor (centered on the editor area).
/// Shows a single-line input with a visible cursor, titled based on
/// whether an existing annotation is being edited.
pub fn render_annotation_prompt(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(prompt) = app.annotation_prompt.as_ref() else {
        return;
    };
    if area.width < 20 || area.height < 5 {
        return;
    }

    let w = 70u16.min(area.width.saturating_sub(4)).max(20);
    let h = 5u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup_area = Rect { x, y, width: w, height: h };

    frame.render_widget(Clear, popup_area);
    let title = if prompt.editing_existing {
        format!(" Edit annotation — line {} ", prompt.line)
    } else {
        format!(" Add annotation — line {} ", prompt.line)
    };
    let border_color = if prompt.editing_existing { Color::Cyan } else { Color::Yellow };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // File path hint (relative to cwd if possible).
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let rel = prompt.path.strip_prefix(&cwd).unwrap_or(&prompt.path);
    let hint = Line::from(Span::styled(
        rel.display().to_string(),
        Style::default().fg(Color::DarkGray),
    ));
    let hint_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(hint), hint_rect);

    // Input line with a block-cursor overlay at `prompt.cursor`.
    if inner.height >= 2 {
        let input_rect = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        let input = &prompt.input;
        let before: String = input[..prompt.cursor].to_string();
        let at: String = input[prompt.cursor..]
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".to_string());
        let after: String = if prompt.cursor < input.len() {
            input[prompt.cursor + at.len()..].to_string()
        } else {
            String::new()
        };
        let spans = vec![
            Span::raw(before),
            Span::styled(
                at,
                Style::default()
                    .bg(Color::White)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(after),
        ];
        frame.render_widget(Paragraph::new(Line::from(spans)), input_rect);
    }

    // Hint footer with keybinds.
    if inner.height >= 3 {
        let foot_rect = Rect {
            x: inner.x,
            y: inner.y + 2,
            width: inner.width,
            height: 1,
        };
        let foot = Line::from(vec![
            Span::styled(
                "Enter",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" save  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "C-d",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" delete  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "C-u",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" clear", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(foot), foot_rect);
    }
}

/// Helper to draw a bordered popup with given text inside `area`.
pub fn draw_popup(frame: &mut Frame<'_>, area: Rect, title: &str, text: &str) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let para = Paragraph::new(text.to_string()).block(block);
    frame.render_widget(para, area);
}
