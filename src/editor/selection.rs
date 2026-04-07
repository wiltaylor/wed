//! Visual selection.

use crate::editor::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Char,
    Line,
    Block,
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub kind: SelectionKind,
    pub anchor: Cursor,
    pub head: Cursor,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            kind: SelectionKind::Char,
            anchor: Cursor::default(),
            head: Cursor::default(),
        }
    }
}

impl Selection {
    pub fn new(kind: SelectionKind, anchor: Cursor, head: Cursor) -> Self {
        Self { kind, anchor, head }
    }

    /// Returns (start, end) ordered (row, col) tuples — inclusive of head as in vim.
    pub fn ordered(&self) -> ((usize, usize), (usize, usize)) {
        let a = (self.anchor.row, self.anchor.col);
        let h = (self.head.row, self.head.col);
        if a <= h {
            (a, h)
        } else {
            (h, a)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_orders() {
        let s = Selection::new(SelectionKind::Char, Cursor::new(2, 3), Cursor::new(1, 0));
        assert_eq!(s.ordered(), ((1, 0), (2, 3)));
    }
}
