// Rope extension helpers.
use ropey::Rope;

/// Helper extension methods for `ropey::Rope`.
pub trait RopeExt {
    /// Return the contents of the given line as a `String`, including any
    /// trailing line terminator.
    fn line_string(&self, line_idx: usize) -> String;
    /// Return the line count clamped to at least 1.
    fn line_count(&self) -> usize;
}

impl RopeExt for Rope {
    fn line_string(&self, line_idx: usize) -> String {
        if line_idx >= self.len_lines() {
            return String::new();
        }
        self.line(line_idx).to_string()
    }

    fn line_count(&self) -> usize {
        self.len_lines().max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_string_basic() {
        let r = Rope::from_str("a\nbb\nccc");
        assert_eq!(r.line_string(1).trim_end(), "bb");
        assert_eq!(r.line_count(), 3);
    }
}
