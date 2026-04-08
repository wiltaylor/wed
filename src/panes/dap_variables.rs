use crate::layout::Pane;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[derive(Debug, Clone)]
pub struct DapVariable {
    pub name: String,
    pub value: String,
    pub ty: Option<String>,
}

#[derive(Default)]
pub struct DapVariablesPane {
    pub variables: Vec<DapVariable>,
    pub selected: usize,
}

impl DapVariablesPane {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_variables(&mut self, variables: Vec<DapVariable>) {
        self.variables = variables;
        if self.selected >= self.variables.len() {
            self.selected = self.variables.len().saturating_sub(1);
        }
    }
}

#[async_trait]
impl Pane for DapVariablesPane {
    fn name(&self) -> &str {
        "dap_variables"
    }
    fn title(&self) -> &str {
        "Variables"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.variables.is_empty() {
            let p = Paragraph::new(Line::from(Span::styled(
                "no variables",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(p, area);
            return;
        }
        let mut lines: Vec<Line> = Vec::with_capacity(self.variables.len());
        for (i, v) in self.variables.iter().enumerate() {
            let ty = v.ty.clone().unwrap_or_default();
            let mut spans = vec![
                Span::styled(
                    format!("{}", v.name),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" = "),
                Span::raw(v.value.clone()),
            ];
            if !ty.is_empty() {
                spans.push(Span::styled(
                    format!("  : {ty}"),
                    Style::default().fg(Color::DarkGray),
                ));
            }
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
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.variables.len() {
                    self.selected += 1;
                }
            }
            _ => {}
        }
    }

    fn row_count(&self) -> usize {
        self.variables.len()
    }
    fn select_row(&mut self, row: usize) {
        if row < self.variables.len() {
            self.selected = row;
        }
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
