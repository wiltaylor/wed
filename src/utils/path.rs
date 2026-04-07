// Path helpers.
use std::path::{Path, PathBuf};

/// Return `path` relative to `base` if possible, else clone `path`.
pub fn relative_to(path: &Path, base: &Path) -> PathBuf {
    path.strip_prefix(base)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Return the file name as a String, or empty string if none.
pub fn file_name_string(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Normalize path to a forward-slashed display string.
pub fn display_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn relative_strips_base() {
        let base = PathBuf::from("/a/b");
        let p = PathBuf::from("/a/b/c/d.rs");
        assert_eq!(relative_to(&p, &base), PathBuf::from("c/d.rs"));
    }

    #[test]
    fn file_name_works() {
        assert_eq!(file_name_string(Path::new("/a/b/c.rs")), "c.rs");
    }
}
