use crate::layout::Pane;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct GitStatusEntry {
    pub path: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GitStatusSummary {
    pub branch: Option<String>,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub entries: Vec<GitStatusEntry>,
}

pub struct GitPane {
    pub root: PathBuf,
    pub status: GitStatusSummary,
}

impl Default for GitPane {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

impl GitPane {
    pub fn new(root: PathBuf) -> Self {
        let mut me = Self {
            root,
            status: GitStatusSummary::default(),
        };
        me.refresh();
        me
    }

    pub fn refresh(&mut self) {
        self.status = read_status(&self.root).unwrap_or_default();
    }
}

/// Read a `GitStatusSummary` for the repo at `root`.
pub fn read_status(root: &Path) -> Option<GitStatusSummary> {
    let repo = git2::Repository::open(root).ok()?;
    let mut summary = GitStatusSummary::default();
    summary.branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts)).ok()?;
    for s in statuses.iter() {
        let st = s.status();
        let mut entry = GitStatusEntry {
            path: s.path().unwrap_or("").to_string(),
            ..Default::default()
        };
        if st.is_index_new()
            || st.is_index_modified()
            || st.is_index_deleted()
            || st.is_index_renamed()
            || st.is_index_typechange()
        {
            entry.staged = true;
            summary.staged += 1;
        }
        if st.is_wt_modified() || st.is_wt_deleted() || st.is_wt_renamed() || st.is_wt_typechange()
        {
            entry.unstaged = true;
            summary.unstaged += 1;
        }
        if st.is_wt_new() {
            entry.untracked = true;
            summary.untracked += 1;
        }
        summary.entries.push(entry);
    }
    Some(summary)
}

#[async_trait]
impl Pane for GitPane {
    fn name(&self) -> &str {
        "git"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_status_for_tempdir_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        // Set a default identity so commits would work; not strictly needed for status.
        let mut cfg = repo.config().unwrap();
        let _ = cfg.set_str("user.name", "test");
        let _ = cfg.set_str("user.email", "test@test");
        fs::write(dir.path().join("new.txt"), "hi").unwrap();
        let summary = read_status(dir.path()).unwrap();
        assert!(summary.untracked >= 1);
        assert!(summary
            .entries
            .iter()
            .any(|e| e.path == "new.txt" && e.untracked));
    }
}
