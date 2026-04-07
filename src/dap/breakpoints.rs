//! Persistent breakpoint store. Stored at `<root>/.wed/breakpoints.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Breakpoint {
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Breakpoint {
    pub fn new(line: u32) -> Self {
        Self {
            line,
            condition: None,
            hit_condition: None,
            log_message: None,
            enabled: true,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BreakpointStore {
    /// File path (as string for portable serialization) -> sorted breakpoints.
    pub files: BTreeMap<PathBuf, Vec<Breakpoint>>,
}

impl BreakpointStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn store_path(root: &Path) -> PathBuf {
        root.join(".wed").join("breakpoints.json")
    }

    /// Load the store from `<root>/.wed/breakpoints.json`. Returns an empty
    /// store if the file does not exist.
    pub fn load(root: &Path) -> Result<Self> {
        let path = Self::store_path(root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(&path)?;
        let store: Self = serde_json::from_slice(&bytes)?;
        Ok(store)
    }

    /// Persist the store to `<root>/.wed/breakpoints.json`.
    pub fn save(&self, root: &Path) -> Result<()> {
        let path = Self::store_path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Toggle a line breakpoint in `file`. Returns `true` if the breakpoint
    /// is now present, `false` if it was removed.
    pub fn toggle(&mut self, file: &Path, line: u32) -> bool {
        let entry = self.files.entry(file.to_path_buf()).or_default();
        if let Some(pos) = entry.iter().position(|b| b.line == line) {
            entry.remove(pos);
            if entry.is_empty() {
                self.files.remove(file);
            }
            false
        } else {
            entry.push(Breakpoint::new(line));
            entry.sort_by_key(|b| b.line);
            true
        }
    }

    /// Get the breakpoints for a file.
    pub fn get(&self, file: &Path) -> &[Breakpoint] {
        self.files.get(file).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn persist_and_reload() {
        let dir = tempdir().unwrap();
        let mut s = BreakpointStore::new();
        let f = PathBuf::from("src/main.rs");
        assert!(s.toggle(&f, 10));
        assert!(s.toggle(&f, 20));
        s.save(dir.path()).unwrap();

        let loaded = BreakpointStore::load(dir.path()).unwrap();
        let bps = loaded.get(&f);
        assert_eq!(bps.len(), 2);
        assert_eq!(bps[0].line, 10);
        assert_eq!(bps[1].line, 20);
        assert!(bps[0].enabled);
    }

    #[test]
    fn load_missing_returns_empty() {
        let dir = tempdir().unwrap();
        let s = BreakpointStore::load(dir.path()).unwrap();
        assert!(s.files.is_empty());
    }

    #[test]
    fn toggle_idempotence() {
        let mut s = BreakpointStore::new();
        let f = PathBuf::from("src/lib.rs");
        assert!(s.toggle(&f, 5)); // add
        assert_eq!(s.get(&f).len(), 1);
        assert!(!s.toggle(&f, 5)); // remove
        assert_eq!(s.get(&f).len(), 0);
        assert!(s.toggle(&f, 5)); // add again -> same state as first add
        assert_eq!(s.get(&f).len(), 1);
        assert_eq!(s.get(&f)[0].line, 5);
        // File entry pruned when emptied.
        s.toggle(&f, 5);
        assert!(s.files.get(&f).is_none());
    }
}
