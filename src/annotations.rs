//! User annotations on files — comment + file + line tuples persisted
//! to `<repo-root>/annotations.txt`. The file is a plain-text, one
//! annotation per line format so it can be handed to AI agents or read
//! by any other tool:
//!
//! ```text
//! path/relative/to/root.rs:42: the comment text
//! ```
//!
//! Paths are stored relative to the repo root when possible. Absolute
//! paths are kept verbatim for files outside the root.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// 1-based line number.
    pub line: u32,
    pub comment: String,
}

#[derive(Debug, Default, Clone)]
pub struct AnnotationStore {
    /// Keyed by canonical absolute path.
    pub files: BTreeMap<PathBuf, Vec<Annotation>>,
}

impl AnnotationStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn store_path(root: &Path) -> PathBuf {
        root.join(".wed").join("annotations.txt")
    }

    /// Load from `<root>/.wed/annotations.txt`. If that file is missing
    /// but a legacy `<root>/annotations.txt` exists, load from the legacy
    /// location (it will migrate automatically on the next save).
    pub fn load(root: &Path) -> Result<Self> {
        let mut path = Self::store_path(root);
        if !path.exists() {
            let legacy = root.join("annotations.txt");
            if legacy.exists() {
                path = legacy;
            } else {
                return Ok(Self::default());
            }
        }
        let text = std::fs::read_to_string(&path)?;
        let mut store = Self::default();
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Split from the right on ": " first to peel off the comment,
            // then find the last `:` before it to peel off the line number.
            let Some((head, comment)) = line.split_once(": ") else {
                continue;
            };
            let Some((path_str, line_str)) = head.rsplit_once(':') else {
                continue;
            };
            let Ok(ln) = line_str.parse::<u32>() else {
                continue;
            };
            let raw = PathBuf::from(path_str);
            let abs = if raw.is_absolute() {
                raw
            } else {
                root.join(&raw)
            };
            let canon = std::fs::canonicalize(&abs).unwrap_or(abs);
            store
                .files
                .entry(canon)
                .or_default()
                .push(Annotation { line: ln, comment: comment.to_string() });
        }
        for v in store.files.values_mut() {
            v.sort_by_key(|a| a.line);
        }
        Ok(store)
    }

    /// Persist to `<root>/.wed/annotations.txt`. Paths are written relative
    /// to `root` when possible.
    pub fn save(&self, root: &Path) -> Result<()> {
        crate::utils::wed_dir::ensure(root)?;
        let path = Self::store_path(root);
        // Clean up the legacy location if we just migrated.
        let legacy = root.join("annotations.txt");
        if legacy.exists() && legacy != path {
            let _ = std::fs::remove_file(&legacy);
        }
        let mut out = String::new();
        for (file, list) in &self.files {
            let rel = pathdiff_relative(file, root).unwrap_or_else(|| file.clone());
            let path_str = rel.to_string_lossy();
            for a in list {
                // Strip any stray newlines in the comment to keep the
                // format one-per-line.
                let comment = a.comment.replace(['\n', '\r'], " ");
                out.push_str(&format!("{}:{}: {}\n", path_str, a.line, comment));
            }
        }
        std::fs::write(path, out)?;
        Ok(())
    }

    /// Add an annotation at `file:line`. Replaces any existing annotation
    /// on the same line (one annotation per line per file).
    pub fn add(&mut self, file: &Path, line: u32, comment: String) {
        let entry = self.files.entry(file.to_path_buf()).or_default();
        if let Some(pos) = entry.iter().position(|a| a.line == line) {
            entry[pos].comment = comment;
        } else {
            entry.push(Annotation { line, comment });
            entry.sort_by_key(|a| a.line);
        }
    }

    /// Remove the annotation at `file:line`. Returns `true` if one was
    /// removed.
    pub fn remove(&mut self, file: &Path, line: u32) -> bool {
        let Some(entry) = self.files.get_mut(file) else {
            return false;
        };
        if let Some(pos) = entry.iter().position(|a| a.line == line) {
            entry.remove(pos);
            if entry.is_empty() {
                self.files.remove(file);
            }
            true
        } else {
            false
        }
    }

    pub fn get(&self, file: &Path) -> &[Annotation] {
        self.files.get(file).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Compute `target` relative to `base` without pulling in a crate.
/// Returns `None` if the paths don't share a common prefix.
fn pathdiff_relative(target: &Path, base: &Path) -> Option<PathBuf> {
    let target = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    let base = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    let t: Vec<_> = target.components().collect();
    let b: Vec<_> = base.components().collect();
    let mut i = 0;
    while i < t.len() && i < b.len() && t[i] == b[i] {
        i += 1;
    }
    // Require base to be a prefix of target; otherwise return absolute.
    if i < b.len() {
        return None;
    }
    let rest: PathBuf = t[i..].iter().collect();
    if rest.as_os_str().is_empty() {
        None
    } else {
        Some(rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn add_remove_and_roundtrip() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        // Create a file inside the root so canonicalization works.
        let f = root.join("src").join("main.rs");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "fn main() {}\n").unwrap();
        let f_canon = std::fs::canonicalize(&f).unwrap();

        let mut s = AnnotationStore::new();
        s.add(&f_canon, 1, "top of file".into());
        s.add(&f_canon, 5, "later".into());
        s.save(&root).unwrap();

        let loaded = AnnotationStore::load(&root).unwrap();
        let list = loaded.get(&f_canon);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].line, 1);
        assert_eq!(list[0].comment, "top of file");
        assert_eq!(list[1].line, 5);

        let mut s2 = loaded;
        assert!(s2.remove(&f_canon, 1));
        assert_eq!(s2.get(&f_canon).len(), 1);
        assert!(!s2.remove(&f_canon, 99));
    }

    #[test]
    fn load_missing_empty() {
        let dir = tempdir().unwrap();
        let s = AnnotationStore::load(dir.path()).unwrap();
        assert!(s.files.is_empty());
    }

    #[test]
    fn add_replaces_same_line() {
        let f = PathBuf::from("/tmp/x.rs");
        let mut s = AnnotationStore::new();
        s.add(&f, 3, "first".into());
        s.add(&f, 3, "second".into());
        assert_eq!(s.get(&f).len(), 1);
        assert_eq!(s.get(&f)[0].comment, "second");
    }
}
