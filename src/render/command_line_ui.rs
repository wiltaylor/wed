use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::input::EditorMode;

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let prefix = match app.mode {
        EditorMode::Command => ":",
        EditorMode::Search => "/",
        _ => return,
    };
    // TODO: pull buffer text from app.command_line once Agent owning input wires it.
    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::Yellow)),
        Span::raw(""),
    ]);
    let para = Paragraph::new(line).style(Style::default().bg(Color::Black).fg(Color::White));
    frame.render_widget(para, area);
}
