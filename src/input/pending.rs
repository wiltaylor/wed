//! Pending input state for multi-key vim sequences.

use crate::editor::Cursor;
use crate::input::mode::Operator;

/// Tracks an in-progress operator/motion command (count + operator).
#[derive(Debug, Clone, Default)]
pub struct PendingState {
    /// Numeric count prefix (None = not yet typed).
    pub count: Option<usize>,
    /// Pending operator (e.g. `d` waiting on a motion).
    pub operator: Option<Operator>,
    /// Buffered chars for sequences like `gg`, `ci"`, etc.
    pub buf: String,
    /// Last find-char (for `;` / `,`).
    pub last_find: Option<(char, bool, bool)>,
    /// Visual mode selection anchor.
    pub visual_anchor: Option<Cursor>,
}

impl PendingState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_digit(&mut self, d: u32) {
        let cur = self.count.unwrap_or(0);
        self.count = Some(cur.saturating_mul(10).saturating_add(d as usize));
    }

    pub fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    pub fn reset(&mut self) {
        self.count = None;
        self.operator = None;
        self.buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_accumulates() {
        let mut p = PendingState::new();
        p.push_digit(3);
        p.push_digit(5);
        assert_eq!(p.take_count(), 35);
        assert_eq!(p.take_count(), 1); // default after take
    }
}
