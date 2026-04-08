//! Git integration: status, stage/unstage, commit, file history.
//!
//! Wraps `git2::Repository`. Synchronous; called from the main thread —
//! operations are bounded (status walks the workdir, history is capped).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use crate::panes::git::{read_status, GitStatusEntry, GitStatusSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileGitStatus {
    Clean,
    Ignored,
    Untracked,
    Modified,
    Staged,
    Deleted,
    Conflicted,
}

impl FileGitStatus {
    /// Priority used when aggregating child statuses to a parent directory.
    /// Higher wins. Unstaged work beats staged work.
    pub fn priority(self) -> u8 {
        match self {
            FileGitStatus::Conflicted => 6,
            FileGitStatus::Deleted => 5,
            FileGitStatus::Modified => 4,
            FileGitStatus::Untracked => 3,
            FileGitStatus::Staged => 2,
            FileGitStatus::Clean => 1,
            FileGitStatus::Ignored => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub oid: String,
    pub short: String,
    pub summary: String,
    pub author: String,
    pub time: String,
}

#[derive(Debug, Default)]
pub struct GitState {
    pub root: PathBuf,
    pub repo_present: bool,
    pub summary: GitStatusSummary,
    /// Absolute path → file status (for the file browser).
    pub status_by_path: HashMap<PathBuf, FileGitStatus>,
}

impl GitState {
    pub fn new(root: PathBuf) -> Self {
        let mut s = Self {
            root,
            ..Default::default()
        };
        s.refresh();
        s
    }

    pub fn refresh(&mut self) {
        let (summary, map) = compute_status(&self.root);
        self.repo_present = summary.is_some();
        self.summary = summary.unwrap_or_default();
        self.status_by_path = map;
    }

    fn open_repo(&self) -> anyhow::Result<git2::Repository> {
        Ok(git2::Repository::discover(&self.root)?)
    }

    /// Stage a path. Works for files (new, modified, deleted) and for
    /// directories (recursively stages everything below).
    pub fn stage(&mut self, abs_path: &Path) -> anyhow::Result<()> {
        let repo = self.open_repo()?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("bare repo"))?
            .to_path_buf();
        let rel = abs_path.strip_prefix(&workdir).unwrap_or(abs_path);
        let mut index = repo.index()?;
        if abs_path.is_dir() {
            // Add new + modified files under the directory, then capture any
            // deletions via update_all on the same pathspec.
            index.add_all([rel], git2::IndexAddOption::DEFAULT, None)?;
            index.update_all([rel], None)?;
        } else if abs_path.exists() {
            index.add_path(rel)?;
        } else {
            index.remove_path(rel)?;
        }
        index.write()?;
        self.refresh();
        Ok(())
    }

    /// Unstage a path: reset it to HEAD's version in the index. If the
    /// working-tree file is missing (i.e. it was deleted), restore it from
    /// HEAD too — "unstage undeletes".
    pub fn unstage(&mut self, abs_path: &Path) -> anyhow::Result<()> {
        let repo = self.open_repo()?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("bare repo"))?
            .to_path_buf();
        let rel = abs_path.strip_prefix(&workdir).unwrap_or(abs_path);
        match repo.head().and_then(|h| h.peel_to_commit()) {
            Ok(head_commit) => {
                let obj = head_commit.into_object();
                repo.reset_default(Some(&obj), [rel])?;
                // If the file is missing from the working tree, force a
                // checkout from HEAD to bring it back.
                if !abs_path.exists() {
                    let mut co = git2::build::CheckoutBuilder::new();
                    co.path(rel).force();
                    repo.checkout_head(Some(&mut co))?;
                }
            }
            Err(_) => {
                // No HEAD yet (initial commit) → just remove from index.
                let mut index = repo.index()?;
                if abs_path.is_dir() {
                    index.remove_dir(rel, 0)?;
                } else {
                    index.remove_path(rel)?;
                }
                index.write()?;
            }
        }
        self.refresh();
        Ok(())
    }

    /// Delete a path from the working tree and stage the deletion. For
    /// directories, removes recursively. Untracked files are simply
    /// removed from disk (no index work needed).
    pub fn delete(&mut self, abs_path: &Path) -> anyhow::Result<()> {
        let repo = self.open_repo()?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("bare repo"))?
            .to_path_buf();
        let rel = abs_path
            .strip_prefix(&workdir)
            .unwrap_or(abs_path)
            .to_path_buf();
        if abs_path.is_dir() {
            std::fs::remove_dir_all(abs_path)?;
        } else if abs_path.exists() {
            std::fs::remove_file(abs_path)?;
        }
        // Stage the deletion (no-op for untracked paths).
        let mut index = repo.index()?;
        index.update_all([&rel], None)?;
        index.write()?;
        self.refresh();
        Ok(())
    }

    /// Create a commit from the current index using the repo's configured signature.
    pub fn commit(&mut self, message: &str) -> anyhow::Result<git2::Oid> {
        if message.trim().is_empty() {
            anyhow::bail!("empty commit message");
        }
        let repo = self.open_repo()?;
        let sig = repo.signature()?;
        let mut index = repo.index()?;
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;
        let parents: Vec<git2::Commit> = match repo.head() {
            Ok(h) => h.peel_to_commit().map(|c| vec![c]).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;
        self.refresh();
        Ok(oid)
    }

    /// Walk commits touching `abs_path`, newest first, capped at `limit`.
    pub fn file_history(&self, abs_path: &Path, limit: usize) -> anyhow::Result<Vec<CommitInfo>> {
        let repo = self.open_repo()?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("bare repo"))?
            .to_path_buf();
        let rel = abs_path
            .strip_prefix(&workdir)
            .unwrap_or(abs_path)
            .to_path_buf();
        let mut walk = repo.revwalk()?;
        walk.push_head()?;
        walk.set_sorting(git2::Sort::TIME)?;
        let mut out = Vec::new();
        for oid in walk {
            if out.len() >= limit {
                break;
            }
            let oid = oid?;
            let commit = repo.find_commit(oid)?;
            // Check if this commit touched `rel` by diffing against the first parent.
            let touched = {
                let tree = commit.tree()?;
                let parent_tree = if commit.parent_count() > 0 {
                    Some(commit.parent(0)?.tree()?)
                } else {
                    None
                };
                let diff =
                    repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
                let mut hit = false;
                diff.foreach(
                    &mut |d, _| {
                        if let Some(p) = d.new_file().path().or_else(|| d.old_file().path()) {
                            if p == rel.as_path() {
                                hit = true;
                            }
                        }
                        true
                    },
                    None,
                    None,
                    None,
                )?;
                hit
            };
            if !touched {
                continue;
            }
            let oid_s = oid.to_string();
            let short = oid_s.chars().take(8).collect();
            let author = commit
                .author()
                .name()
                .unwrap_or("?")
                .to_string();
            let summary = commit.summary().unwrap_or("").to_string();
            let time = format_git_time(commit.time());
            out.push(CommitInfo {
                oid: oid_s,
                short,
                summary,
                author,
                time,
            });
        }
        Ok(out)
    }
}

/// Run the synchronous git status walk and return the summary plus the
/// absolute-path → status map. `summary` is `None` when no repo is present.
pub fn compute_status(
    root: &Path,
) -> (Option<GitStatusSummary>, HashMap<PathBuf, FileGitStatus>) {
    let summary = match read_status(root) {
        Some(s) => s,
        None => return (None, HashMap::new()),
    };
    let workdir = git2::Repository::discover(root)
        .ok()
        .and_then(|r| r.workdir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| root.to_path_buf());
    let mut map = HashMap::new();
    for e in &summary.entries {
        let abs = workdir.join(&e.path);
        let status = if e.deleted {
            FileGitStatus::Deleted
        } else if e.staged && e.unstaged {
            FileGitStatus::Modified
        } else if e.staged {
            FileGitStatus::Staged
        } else if e.unstaged {
            FileGitStatus::Modified
        } else if e.untracked {
            FileGitStatus::Untracked
        } else {
            FileGitStatus::Clean
        };
        map.insert(abs, status);
    }
    (Some(summary), map)
}

/// Spawn a background task that runs `compute_status` off the main thread
/// and posts an `AppEvent::GitStatusUpdated` when done.
pub fn spawn_refresh(
    root: PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<crate::app::AppEvent>,
) {
    tokio::task::spawn_blocking(move || {
        let (summary, status_by_path) = compute_status(&root);
        let _ = tx.send(crate::app::AppEvent::GitStatusUpdated {
            summary: summary.unwrap_or_default(),
            status_by_path,
        });
    });
}

fn format_git_time(t: git2::Time) -> String {
    // Simple YYYY-MM-DD HH:MM from seconds since epoch.
    let secs = t.seconds();
    // git2 doesn't pull in chrono; do a tiny manual conversion.
    let days = secs.div_euclid(86_400);
    let seconds_of_day = secs.rem_euclid(86_400);
    let hh = seconds_of_day / 3600;
    let mm = (seconds_of_day % 3600) / 60;
    // Days since 1970-01-01 → calendar date.
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02} {hh:02}:{mm:02}")
}

fn days_to_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    // Algorithm from Howard Hinnant: civil_from_days.
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn init_repo() -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "test").unwrap();
        cfg.set_str("user.email", "test@test").unwrap();
        (dir, repo)
    }

    #[test]
    fn stage_then_unstage_round_trip() {
        let (dir, _repo) = init_repo();
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let mut g = GitState::new(dir.path().to_path_buf());
        let abs = dir.path().join("a.txt");
        g.stage(&abs).unwrap();
        let st = *g.status_by_path.get(&abs).unwrap();
        assert_eq!(st, FileGitStatus::Staged);
        g.unstage(&abs).unwrap();
        let st = *g.status_by_path.get(&abs).unwrap();
        assert_eq!(st, FileGitStatus::Untracked);
    }

    #[test]
    fn commit_creates_head() {
        let (dir, _repo) = init_repo();
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let mut g = GitState::new(dir.path().to_path_buf());
        g.stage(&dir.path().join("a.txt")).unwrap();
        let oid = g.commit("first commit").unwrap();
        assert!(!oid.is_zero());
        // After commit, file should be clean (not in status map).
        assert!(g
            .status_by_path
            .get(&dir.path().join("a.txt"))
            .is_none());
    }

    #[test]
    fn file_history_lists_commits() {
        let (dir, _repo) = init_repo();
        let p = dir.path().join("a.txt");
        let mut g = GitState::new(dir.path().to_path_buf());
        fs::write(&p, "v1").unwrap();
        g.stage(&p).unwrap();
        g.commit("v1").unwrap();
        fs::write(&p, "v2").unwrap();
        g.stage(&p).unwrap();
        g.commit("v2").unwrap();
        let hist = g.file_history(&p, 10).unwrap();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].summary, "v2");
        assert_eq!(hist[1].summary, "v1");
    }
}
