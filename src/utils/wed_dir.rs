//! Helpers for the per-repo `.wed/` directory where wed stores its
//! persistent state (breakpoints, annotations, …).

use std::path::{Path, PathBuf};

/// Return `<root>/.wed`, ensuring the directory exists and contains a
/// `.gitignore` that excludes its entire contents.
pub fn ensure(root: &Path) -> std::io::Result<PathBuf> {
    let dir = root.join(".wed");
    std::fs::create_dir_all(&dir)?;
    let gi = dir.join(".gitignore");
    if !gi.exists() {
        std::fs::write(&gi, "*\n")?;
    }
    Ok(dir)
}
