//! Cursor with column-affinity tracking.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    /// Desired column for vertical motions; survives moving through short lines.
    pub want_col: usize,
}

impl Cursor {
    pub fn new(row: usize, col: usize) -> Self {
        Self {
            row,
            col,
            want_col: col,
        }
    }

    /// Set position and reset column affinity.
    pub fn set(&mut self, row: usize, col: usize) {
        self.row = row;
        self.col = col;
        self.want_col = col;
    }

    /// Move vertically while preserving affinity.
    pub fn set_row_keep_want(&mut self, row: usize, max_col: usize) {
        self.row = row;
        self.col = self.want_col.min(max_col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_set_resets_want() {
        let mut c = Cursor::new(0, 0);
        c.set(2, 5);
        assert_eq!(c.want_col, 5);
    }

    #[test]
    fn cursor_keep_want_clamps() {
        let mut c = Cursor::new(0, 10);
        c.set_row_keep_want(1, 3);
        assert_eq!(c.col, 3);
        assert_eq!(c.want_col, 10);
    }
}
