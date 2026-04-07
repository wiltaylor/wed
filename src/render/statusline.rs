use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::input::EditorMode;

fn mode_label(mode: EditorMode) -> (&'static str, Color) {
    match mode {
        EditorMode::Normal => ("NORMAL", Color::Blue),
        EditorMode::Insert => ("INSERT", Color::Green),
        EditorMode::Visual(_) => ("VISUAL", Color::Magenta),
        EditorMode::Replace => ("REPLACE", Color::Red),
        EditorMode::Command => ("COMMAND", Color::Yellow),
        EditorMode::Search => ("SEARCH", Color::Yellow),
        EditorMode::Pending(_) => ("PENDING", Color::DarkGray),
        EditorMode::Operator(_) => ("OPERATOR", Color::Cyan),
    }
}

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let (mode_text, mode_color) = mode_label(app.mode);

    let (file_name, dirty, lang, row, col, percent) = {
        let mut file_name = String::from("[no name]");
        let mut dirty = false;
        let mut lang = String::new();
        let mut row = 0usize;
        let mut col = 0usize;
        let mut percent = 0u16;
        if let Some(tab) = app.layout.active_tab() {
            if let Some(view) = tab.root.find(tab.active_view) {
                row = view.cursor.0;
                col = view.cursor.1;
                if let Some(buf) = app.buffers.iter().find(|b| b.id == view.buffer_id) {
                    if let Some(p) = &buf.path {
                        file_name = p
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| p.display().to_string());
                    }
                    dirty = buf.dirty;
                    lang = buf.language_id.clone().unwrap_or_default();
                    let total = buf.rope.len_lines().max(1);
                    percent = ((row + 1) * 100 / total).min(100) as u16;
                }
            }
        }
        (file_name, dirty, lang, row, col, percent)
    };

    let dirty_marker = if dirty { " [+]" } else { "" };
    let left = vec![
        Span::styled(
            format!(" {mode_text} "),
            Style::default().bg(mode_color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(file_name, Style::default().fg(Color::White)),
        Span::styled(dirty_marker, Style::default().fg(Color::Yellow)),
    ];
    let right_text = format!(" {lang}  {}:{}  {}% ", row + 1, col + 1, percent);

    let total_w = area.width as usize;
    let left_w: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let pad = total_w.saturating_sub(left_w + right_text.chars().count());
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(right_text, Style::default().fg(Color::Gray)));

    let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(para, area);
}
