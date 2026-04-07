use crate::layout::Pane;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub snippet: String,
}

#[derive(Default)]
pub struct SearchResultsPane {
    pub hits: Vec<SearchHit>,
    pub selected: usize,
}

impl SearchResultsPane {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_hits(&mut self, hits: Vec<SearchHit>) {
        self.hits = hits;
        self.selected = 0;
    }
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.hits.len() {
            self.selected += 1;
        }
    }
}

#[async_trait]
impl Pane for SearchResultsPane {
    fn name(&self) -> &str {
        "search_results"
    }
}

/// Project search: walk `root` (respecting .gitignore), match each line against
/// `pattern`, and return the resulting hits.
pub fn search_project(root: &Path, pattern: &str) -> anyhow::Result<Vec<SearchHit>> {
    let re = regex::Regex::new(pattern)?;
    let mut hits = Vec::new();
    for entry in ignore::Walk::new(root).flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for (lineno, line) in content.lines().enumerate() {
            if let Some(m) = re.find(line) {
                hits.push(SearchHit {
                    path: path.to_path_buf(),
                    line: lineno + 1,
                    col: m.start() + 1,
                    snippet: line.to_string(),
                });
            }
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_regex_hits() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "foo\nbar baz\nfoobar\n").unwrap();
        fs::write(dir.path().join("b.txt"), "nothing").unwrap();
        let hits = search_project(dir.path(), r"foo").unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.path.file_name().unwrap() == "a.txt"));
    }
}
