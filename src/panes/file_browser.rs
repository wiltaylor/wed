use crate::layout::Pane;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// One row in the file browser.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
}

/// Tree-style file browser pane.
pub struct FileBrowserPane {
    pub root: PathBuf,
    pub entries: Vec<FileEntry>,
    pub expanded: HashSet<PathBuf>,
    pub selected: usize,
    pub last_opened: Option<PathBuf>,
}

impl Default for FileBrowserPane {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

impl FileBrowserPane {
    pub fn new(root: PathBuf) -> Self {
        let mut me = Self {
            root,
            entries: Vec::new(),
            expanded: HashSet::new(),
            selected: 0,
            last_opened: None,
        };
        me.refresh();
        me
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        let root = self.root.clone();
        for entry in ignore::WalkBuilder::new(&root)
            .hidden(false)
            .build()
            .flatten()
        {
            let depth = entry.depth();
            let path = entry.path().to_path_buf();
            if path == root {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            self.entries.push(FileEntry {
                path,
                depth,
                is_dir,
            });
        }
    }

    pub fn visible(&self) -> Vec<&FileEntry> {
        let mut out = Vec::new();
        let mut skip_depth: Option<usize> = None;
        for e in &self.entries {
            if let Some(d) = skip_depth {
                if e.depth > d {
                    continue;
                } else {
                    skip_depth = None;
                }
            }
            out.push(e);
            if e.is_dir && !self.expanded.contains(&e.path) {
                skip_depth = Some(e.depth);
            }
        }
        out
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.visible().get(self.selected).map(|e| e.path.clone())
    }

    pub fn move_down(&mut self) {
        let len = self.visible().len();
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn activate(&mut self) {
        let path = match self.selected_path() {
            Some(p) => p,
            None => return,
        };
        if path.is_dir() {
            if !self.expanded.insert(path.clone()) {
                self.expanded.remove(&path);
            }
        } else {
            self.last_opened = Some(path);
        }
    }

    pub(crate) fn _root_ref(&self) -> &Path {
        &self.root
    }
}

#[async_trait]
impl Pane for FileBrowserPane {
    fn name(&self) -> &str {
        "file_browser"
    }
    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let visible = self.visible();
        let height = area.height as usize;
        let start = self.selected.saturating_sub(height.saturating_sub(1));
        let lines: Vec<Line> = visible
            .iter()
            .enumerate()
            .skip(start)
            .take(height)
            .map(|(i, e)| {
                let name = e
                    .path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let glyph = if e.is_dir {
                    if self.expanded.contains(&e.path) {
                        "▾ "
                    } else {
                        "▸ "
                    }
                } else {
                    "  "
                };
                let indent = "  ".repeat(e.depth.saturating_sub(1));
                let text = format!("{indent}{glyph}{name}");
                let mut style = Style::default().fg(if e.is_dir {
                    Color::Cyan
                } else {
                    Color::Gray
                });
                if i == self.selected {
                    style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
                }
                Line::from(Span::styled(text, style))
            })
            .collect();
        let para = Paragraph::new(lines);
        frame.render_widget(para, area);
    }
    fn take_opened_path(&mut self) -> Option<PathBuf> {
        self.last_opened.take()
    }
    fn row_count(&self) -> usize {
        self.visible().len()
    }
    fn select_row(&mut self, row: usize) {
        let len = self.visible().len();
        if len == 0 {
            return;
        }
        self.selected = row.min(len - 1);
    }
    fn activate_selected(&mut self) {
        self.activate();
    }
    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Enter => self.activate(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn walks_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b.txt"), "x").unwrap();
        let pane = FileBrowserPane::new(dir.path().to_path_buf());
        let names: Vec<_> = pane
            .entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"sub".to_string()));
        assert!(names.contains(&"b.txt".to_string()));
    }
}
