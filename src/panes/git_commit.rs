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
    pub staged: Vec<(String, bool)>,
    pub message: String,
    pub focus: CommitFocus,
    /// Byte offset into `message`.
    pub cursor: usize,
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
            cursor: 0,
            pending_commit: false,
        }
    }
}

fn prev_char_boundary(s: &str, i: usize) -> usize {
    let mut j = i.saturating_sub(1);
    while j > 0 && !s.is_char_boundary(j) {
        j -= 1;
    }
    j
}

fn next_char_boundary(s: &str, i: usize) -> usize {
    let mut j = (i + 1).min(s.len());
    while j < s.len() && !s.is_char_boundary(j) {
        j += 1;
    }
    j
}

/// Row (lines before cursor) and column (chars since last newline).
fn cursor_rowcol(s: &str, cursor: usize) -> (usize, usize) {
    let before = &s[..cursor];
    let row = before.bytes().filter(|b| *b == b'\n').count();
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = s[line_start..cursor].chars().count();
    (row, col)
}

fn line_bounds(s: &str, cursor: usize) -> (usize, usize) {
    let start = s[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = s[cursor..]
        .find('\n')
        .map(|i| cursor + i)
        .unwrap_or(s.len());
    (start, end)
}

impl GitCommitPane {
    pub fn set_staged(&mut self, list: Vec<(String, bool)>) {
        self.staged = list;
    }

    pub fn take_pending_commit(&mut self) -> Option<String> {
        if self.pending_commit && !self.message.trim().is_empty() {
            self.pending_commit = false;
            self.cursor = 0;
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
    fn refresh_staged(&mut self, staged: &[(String, bool)]) {
        self.staged = staged.to_vec();
    }
    fn take_commit_request(&mut self) -> Option<String> {
        self.take_pending_commit()
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        self.render_focused(frame, area, false);
    }
    fn render_focused(&self, frame: &mut Frame<'_>, area: Rect, panel_focused: bool) {
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
                .map(|(f, deleted)| {
                    let color = if *deleted { Color::Red } else { Color::LightGreen };
                    Line::from(Span::styled(f.clone(), Style::default().fg(color)))
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
        if panel_focused && msg_active && msg_inner.width > 0 && msg_inner.height > 0 {
            let (row, col) = cursor_rowcol(&self.message, self.cursor.min(self.message.len()));
            let cx = msg_inner.x + (col as u16).min(msg_inner.width.saturating_sub(1));
            let cy = msg_inner.y + (row as u16).min(msg_inner.height.saturating_sub(1));
            frame.set_cursor_position((cx, cy));
        }

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
            CommitFocus::Message => {
                if self.cursor > self.message.len() {
                    self.cursor = self.message.len();
                }
                match key.code {
                    KeyCode::Char(c) => {
                        let mut buf = [0u8; 4];
                        let s = c.encode_utf8(&mut buf);
                        self.message.insert_str(self.cursor, s);
                        self.cursor += s.len();
                    }
                    KeyCode::Enter => {
                        self.message.insert(self.cursor, '\n');
                        self.cursor += 1;
                    }
                    KeyCode::Backspace => {
                        if self.cursor > 0 {
                            let prev = prev_char_boundary(&self.message, self.cursor);
                            self.message.drain(prev..self.cursor);
                            self.cursor = prev;
                        }
                    }
                    KeyCode::Delete => {
                        if self.cursor < self.message.len() {
                            let next = next_char_boundary(&self.message, self.cursor);
                            self.message.drain(self.cursor..next);
                        }
                    }
                    KeyCode::Left => {
                        if self.cursor > 0 {
                            self.cursor = prev_char_boundary(&self.message, self.cursor);
                        }
                    }
                    KeyCode::Right => {
                        if self.cursor < self.message.len() {
                            self.cursor = next_char_boundary(&self.message, self.cursor);
                        }
                    }
                    KeyCode::Home => {
                        self.cursor = line_bounds(&self.message, self.cursor).0;
                    }
                    KeyCode::End => {
                        self.cursor = line_bounds(&self.message, self.cursor).1;
                    }
                    KeyCode::Up => {
                        let (start, _) = line_bounds(&self.message, self.cursor);
                        if start > 0 {
                            let col = self.message[start..self.cursor].chars().count();
                            let prev_end = start - 1;
                            let prev_start = self.message[..prev_end]
                                .rfind('\n')
                                .map(|i| i + 1)
                                .unwrap_or(0);
                            let mut idx = prev_start;
                            let slice = &self.message[prev_start..prev_end];
                            for (n, (bi, _)) in slice.char_indices().enumerate() {
                                if n == col {
                                    idx = prev_start + bi;
                                    break;
                                }
                                idx = prev_start + bi + slice[bi..].chars().next().unwrap().len_utf8();
                            }
                            self.cursor = idx.min(prev_end);
                        }
                    }
                    KeyCode::Down => {
                        let (start, end) = line_bounds(&self.message, self.cursor);
                        if end < self.message.len() {
                            let col = self.message[start..self.cursor].chars().count();
                            let next_start = end + 1;
                            let next_end = self.message[next_start..]
                                .find('\n')
                                .map(|i| next_start + i)
                                .unwrap_or(self.message.len());
                            let mut idx = next_start;
                            let slice = &self.message[next_start..next_end];
                            for (n, (bi, ch)) in slice.char_indices().enumerate() {
                                if n == col {
                                    idx = next_start + bi;
                                    break;
                                }
                                idx = next_start + bi + ch.len_utf8();
                            }
                            self.cursor = idx.min(next_end);
                        }
                    }
                    _ => {}
                }
            }
            CommitFocus::Button => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.pending_commit = true;
                }
            }
        }
    }
}
