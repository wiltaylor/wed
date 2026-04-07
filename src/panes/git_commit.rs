//! Bottom-panel pane: list staged files, edit a commit message, commit.
//!
//! Commit execution lives outside the pane (the pane has no `App` access);
//! the host inspects `take_commit_request` after dispatching keys.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::layout::Pane;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitFocus {
    Files,
    Message,
    Button,
}

pub struct GitCommitPane {
    pub staged: Vec<String>,
    pub message: String,
    pub focus: CommitFocus,
    /// Set to true when the user activates the Commit button. The host
    /// drains this via `take_commit_request` and runs the commit.
    pending_commit: bool,
}

impl Default for GitCommitPane {
    fn default() -> Self {
        Self {
            staged: Vec::new(),
            message: String::new(),
            focus: CommitFocus::Message,
            pending_commit: false,
        }
    }
}

impl GitCommitPane {
    pub fn set_staged(&mut self, list: Vec<String>) {
        self.staged = list;
    }

    pub fn take_pending_commit(&mut self) -> Option<String> {
        if self.pending_commit && !self.message.trim().is_empty() {
            self.pending_commit = false;
            Some(std::mem::take(&mut self.message))
        } else {
            self.pending_commit = false;
            None
        }
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            CommitFocus::Files => CommitFocus::Message,
            CommitFocus::Message => CommitFocus::Button,
            CommitFocus::Button => CommitFocus::Files,
        };
    }
}

impl Pane for GitCommitPane {
    fn name(&self) -> &str {
        "commit"
    }
    fn title(&self) -> &str {
        "Commit"
    }
    fn refresh_staged(&mut self, staged: &[String]) {
        self.staged = staged.to_vec();
    }
    fn take_commit_request(&mut self) -> Option<String> {
        self.take_pending_commit()
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length((self.staged.len() as u16 + 2).min(area.height / 2).max(3)),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(area);

        // Staged files
        let files_active = self.focus == CommitFocus::Files;
        let files_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if files_active {
                Color::Yellow
            } else {
                Color::DarkGray
            }))
            .title(" Staged ");
        let files_inner = files_block.inner(chunks[0]);
        frame.render_widget(files_block, chunks[0]);
        if self.staged.is_empty() {
            let p = Paragraph::new(Line::from(Span::styled(
                "(nothing staged)",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(p, files_inner);
        } else {
            let lines: Vec<Line> = self
                .staged
                .iter()
                .map(|f| {
                    Line::from(Span::styled(
                        f.clone(),
                        Style::default().fg(Color::LightGreen),
                    ))
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), files_inner);
        }

        // Message box
        let msg_active = self.focus == CommitFocus::Message;
        let msg_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if msg_active {
                Color::Yellow
            } else {
                Color::DarkGray
            }))
            .title(" Message ");
        let msg_inner = msg_block.inner(chunks[1]);
        frame.render_widget(msg_block, chunks[1]);
        let msg_lines: Vec<Line> = if self.message.is_empty() {
            vec![Line::from(Span::styled(
                "<type your commit message — Tab to switch focus>",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            self.message
                .lines()
                .map(|l| Line::from(Span::raw(l.to_string())))
                .collect()
        };
        frame.render_widget(Paragraph::new(msg_lines), msg_inner);

        // Button
        let btn_active = self.focus == CommitFocus::Button;
        let btn_style = if btn_active {
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        };
        let label = if self.staged.is_empty() {
            " [ Commit (nothing staged) ] "
        } else {
            " [ Commit ] "
        };
        let p = Paragraph::new(Line::from(Span::styled(label, btn_style)));
        frame.render_widget(p, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Tab is handled by the host (cycles bottom-panel tabs); we use BackTab
        // / Ctrl-cycle here. Use Down/Up to cycle focus too for ergonomics.
        match key.code {
            KeyCode::BackTab => self.cycle_focus(),
            _ => {}
        }
        match self.focus {
            CommitFocus::Files => match key.code {
                KeyCode::Down | KeyCode::Up => {}
                KeyCode::Enter => self.cycle_focus(),
                _ => {}
            },
            CommitFocus::Message => match key.code {
                KeyCode::Char(c) => self.message.push(c),
                KeyCode::Enter => self.message.push('\n'),
                KeyCode::Backspace => {
                    self.message.pop();
                }
                _ => {}
            },
            CommitFocus::Button => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.pending_commit = true;
                }
            }
        }
    }
}
