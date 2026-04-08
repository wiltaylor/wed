use crate::layout::Pane;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub name: String,
    pub source: Option<String>,
    pub line: usize,
}

#[derive(Default)]
pub struct DapCallStackPane {
    pub frames: Vec<StackFrame>,
    pub selected: usize,
    pending_jump: Option<(usize, usize)>,
}

impl DapCallStackPane {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_frames(&mut self, frames: Vec<StackFrame>) {
        self.frames = frames;
        self.selected = 0;
    }
}

pub type DapCallstackPane = DapCallStackPane;

#[async_trait]
impl Pane for DapCallStackPane {
    fn name(&self) -> &str {
        "dap_callstack"
    }
    fn title(&self) -> &str {
        "Call Stack"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.frames.is_empty() {
            let p = Paragraph::new(Line::from(Span::styled(
                "no stack",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(p, area);
            return;
        }
        let mut lines: Vec<Line> = Vec::with_capacity(self.frames.len());
        for (i, f) in self.frames.iter().enumerate() {
            let src = f.source.clone().unwrap_or_default();
            let prefix = format!("#{i:<2} {}  ", f.name);
            let suffix = format!("{}:{}", src, f.line + 1);
            let mut spans = vec![
                Span::styled(prefix, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(suffix, Style::default().fg(Color::DarkGray)),
            ];
            let style = if i == self.selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            for s in &mut spans {
                s.style = s.style.patch(style);
            }
            lines.push(Line::from(spans));
        }
        let visible = area.height as usize;
        let start = if self.selected >= visible {
            self.selected + 1 - visible
        } else {
            0
        };
        let view: Vec<Line> = lines.into_iter().skip(start).take(visible).collect();
        frame.render_widget(Paragraph::new(view), area);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                if let Some(f) = self.frames.get(self.selected) {
                    self.pending_jump = Some((f.line, 0));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.frames.len() {
                    self.selected += 1;
                }
                if let Some(f) = self.frames.get(self.selected) {
                    self.pending_jump = Some((f.line, 0));
                }
            }
            KeyCode::Enter => {
                if let Some(f) = self.frames.get(self.selected) {
                    self.pending_jump = Some((f.line, 0));
                }
            }
            _ => {}
        }
    }

    fn row_count(&self) -> usize {
        self.frames.len()
    }
    fn select_row(&mut self, row: usize) {
        if row < self.frames.len() {
            self.selected = row;
        }
    }
    fn activate_selected(&mut self) {
        if let Some(f) = self.frames.get(self.selected) {
            self.pending_jump = Some((f.line, 0));
        }
    }
    fn take_jump_target(&mut self) -> Option<(usize, usize)> {
        self.pending_jump.take()
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
