use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::input::EditorMode;

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let line = match app.mode {
        EditorMode::Command => Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::raw(app.command_line.input.clone()),
        ]),
        EditorMode::Search => Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(app.command_line.input.clone()),
        ]),
        _ => match &app.status_message {
            Some((msg, is_err)) => {
                let color = if *is_err { Color::Red } else { Color::White };
                Line::from(Span::styled(msg.clone(), Style::default().fg(color)))
            }
            None => return,
        },
    };
    let para = Paragraph::new(line).style(Style::default().bg(Color::Black).fg(Color::White));
    frame.render_widget(para, area);
}
