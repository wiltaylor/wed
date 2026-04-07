use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::input::EditorMode;

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let prefix = match app.mode {
        EditorMode::Command => ":",
        EditorMode::Search => "/",
        _ => return,
    };
    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::Yellow)),
        Span::raw(app.command_line.input.clone()),
    ]);
    let para = Paragraph::new(line).style(Style::default().bg(Color::Black).fg(Color::White));
    frame.render_widget(para, area);
}
