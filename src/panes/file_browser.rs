use crate::layout::Pane;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::git::FileGitStatus;

/// One row in the file browser.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub ignored: bool,
}

/// Tree-style file browser pane.
pub struct FileBrowserPane {
    pub root: PathBuf,
    pub entries: Vec<FileEntry>,
    pub expanded: HashSet<PathBuf>,
    pub selected: usize,
    pub last_opened: Option<PathBuf>,
    pub git_status: HashMap<PathBuf, FileGitStatus>,
    /// Cached effective status (file → status, plus aggregated parents).
    /// Recomputed when entries or git_status change, NOT every render.
    effective_cache: HashMap<PathBuf, FileGitStatus>,
    /// Paths inserted as "phantom" entries because git reports them as
    /// deleted but they're not on disk. Tracked so we can clean them up
    /// when git status changes (e.g. after a restore).
    phantom_paths: HashSet<PathBuf>,
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
            git_status: HashMap::new(),
            effective_cache: HashMap::new(),
            phantom_paths: HashSet::new(),
        };
        me.refresh();
        me
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        let root = self.root.clone();
        // Open the repo (if any) so we can classify ignored paths. We walk
        // everything (ignore rules disabled in the walker) and defer the
        // ignore decision to git itself.
        let repo = git2::Repository::discover(&root).ok();
        let workdir = repo
            .as_ref()
            .and_then(|r| r.workdir().map(|p| p.to_path_buf()));
        for entry in ignore::WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(false)
            .git_exclude(false)
            .git_global(false)
            .ignore(false)
            .parents(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build()
            .flatten()
        {
            let depth = entry.depth();
            let path = entry.path().to_path_buf();
            if path == root {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let ignored = match (repo.as_ref(), workdir.as_ref()) {
                (Some(r), Some(wd)) => path
                    .strip_prefix(wd)
                    .ok()
                    .and_then(|rel| r.status_should_ignore(rel).ok())
                    .unwrap_or(false),
                _ => false,
            };
            self.entries.push(FileEntry {
                path,
                depth,
                is_dir,
                ignored,
            });
        }
        // Walking the disk dropped any phantoms; rebuild them from
        // whatever git_status we already have.
        self.phantom_paths.clear();
        self.inject_deleted_entries();
        self.rebuild_effective_cache();
    }

    /// Build a map from path → effective git status, with directories
    /// reflecting the highest-priority status found among their descendants.
    /// For each path in `git_status` marked Deleted that lives under our
    /// root and isn't already in `entries`, insert a phantom entry so the
    /// user can still see (and act on) the file.
    fn inject_deleted_entries(&mut self) {
        // Drop any phantoms from the previous pass first.
        if !self.phantom_paths.is_empty() {
            let phantoms = std::mem::take(&mut self.phantom_paths);
            self.entries.retain(|e| !phantoms.contains(&e.path));
        }
        let mut to_add: Vec<PathBuf> = self
            .git_status
            .iter()
            .filter_map(|(p, s)| {
                if matches!(s, FileGitStatus::Deleted)
                    && p.starts_with(&self.root)
                    && !self.entries.iter().any(|e| &e.path == p)
                {
                    Some(p.clone())
                } else {
                    None
                }
            })
            .collect();
        // Stable insertion order so re-renders don't shuffle.
        to_add.sort();
        for path in to_add {
            let depth = path
                .strip_prefix(&self.root)
                .map(|p| p.components().count())
                .unwrap_or(1);
            // Find insertion point: after the last existing descendant of
            // this file's parent directory, so it slots into the tree where
            // it would have been if it still existed on disk.
            let parent = path.parent();
            let mut insert_at = self.entries.len();
            if let Some(parent) = parent {
                if parent != self.root.as_path() {
                    if let Some(parent_idx) =
                        self.entries.iter().position(|e| e.path == parent)
                    {
                        let parent_depth = self.entries[parent_idx].depth;
                        insert_at = parent_idx + 1;
                        while insert_at < self.entries.len()
                            && self.entries[insert_at].depth > parent_depth
                        {
                            insert_at += 1;
                        }
                    }
                }
            }
            self.phantom_paths.insert(path.clone());
            self.entries.insert(
                insert_at,
                FileEntry {
                    path,
                    depth,
                    is_dir: false,
                    ignored: false,
                },
            );
        }
    }

    fn rebuild_effective_cache(&mut self) {
        let mut eff: HashMap<PathBuf, FileGitStatus> = HashMap::new();
        for e in &self.entries {
            if e.is_dir {
                continue;
            }
            let s = self.git_status.get(&e.path).copied().unwrap_or({
                if e.ignored {
                    FileGitStatus::Ignored
                } else {
                    FileGitStatus::Clean
                }
            });
            eff.insert(e.path.clone(), s);
            // Bubble up to ancestors within the root.
            let mut cur = e.path.as_path();
            while let Some(parent) = cur.parent() {
                if !parent.starts_with(&self.root) {
                    break;
                }
                if parent == self.root.as_path() {
                    break;
                }
                let slot = eff.entry(parent.to_path_buf()).or_insert(FileGitStatus::Ignored);
                if s.priority() > slot.priority() {
                    *slot = s;
                }
                cur = parent;
            }
        }
        self.effective_cache = eff;
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

    pub fn set_git_status(&mut self, map: HashMap<PathBuf, FileGitStatus>) {
        self.git_status = map;
        self.inject_deleted_entries();
        self.rebuild_effective_cache();
    }
}

#[async_trait]
impl Pane for FileBrowserPane {
    fn name(&self) -> &str {
        "file_browser"
    }
    fn path_at_row(&self, row: usize) -> Option<PathBuf> {
        self.visible().get(row).map(|e| e.path.clone())
    }
    fn refresh_git_status(
        &mut self,
        map: &HashMap<PathBuf, crate::git::FileGitStatus>,
    ) {
        self.git_status = map.clone();
        self.inject_deleted_entries();
        self.rebuild_effective_cache();
    }
    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let visible = self.visible();
        let eff = &self.effective_cache;
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
                let status = eff.get(&e.path).copied().unwrap_or(FileGitStatus::Clean);
                let fg = match status {
                    FileGitStatus::Conflicted => Color::Red,
                    FileGitStatus::Deleted => Color::Red,
                    FileGitStatus::Modified => Color::Yellow,
                    FileGitStatus::Untracked => Color::Green,
                    FileGitStatus::Staged => Color::LightGreen,
                    FileGitStatus::Ignored => Color::DarkGray,
                    FileGitStatus::Clean => {
                        if e.is_dir {
                            Color::Cyan
                        } else {
                            Color::Gray
                        }
                    }
                };
                let mut style = Style::default().fg(fg);
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
