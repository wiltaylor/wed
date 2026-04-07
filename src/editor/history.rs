//! Linear undo history with insert-mode batching.
//!
//! Each entry records a byte-range replacement so it can be inverted.

#[derive(Debug, Clone)]
pub struct EditOp {
    pub start: usize,
    /// What was at `start..start+removed.len()` before the edit.
    pub removed: String,
    /// What was inserted in its place.
    pub inserted: String,
    /// Cursor position (byte offset) before the edit, for undo restore.
    pub cursor_before: usize,
    /// Cursor position (byte offset) after the edit, for redo restore.
    pub cursor_after: usize,
}

#[derive(Debug, Clone, Default)]
pub struct EditBatch {
    pub ops: Vec<EditOp>,
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<EditBatch>,
    redo: Vec<EditBatch>,
    /// Open batch (insert-mode session). When `Some`, new ops accumulate here.
    open: Option<EditBatch>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a new batch (e.g. entering insert mode).
    pub fn begin_batch(&mut self) {
        self.commit_batch();
        self.open = Some(EditBatch::default());
    }

    /// Close the open batch and push it onto the undo stack.
    pub fn commit_batch(&mut self) {
        if let Some(b) = self.open.take() {
            if !b.ops.is_empty() {
                self.undo.push(b);
                self.redo.clear();
            }
        }
    }

    /// Record a single op. If a batch is open, append; otherwise push as its own batch.
    pub fn record(&mut self, op: EditOp) {
        if let Some(b) = self.open.as_mut() {
            b.ops.push(op);
        } else {
            self.undo.push(EditBatch { ops: vec![op] });
            self.redo.clear();
        }
    }

    pub fn pop_undo(&mut self) -> Option<EditBatch> {
        self.commit_batch();
        let b = self.undo.pop()?;
        self.redo.push(b.clone());
        Some(b)
    }

    pub fn pop_redo(&mut self) -> Option<EditBatch> {
        let b = self.redo.pop()?;
        self.undo.push(b.clone());
        Some(b)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.open.as_ref().map_or(false, |b| !b.ops.is_empty())
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(s: usize, rem: &str, ins: &str) -> EditOp {
        EditOp {
            start: s,
            removed: rem.into(),
            inserted: ins.into(),
            cursor_before: s,
            cursor_after: s + ins.len(),
        }
    }

    #[test]
    fn batching_and_undo_redo() {
        let mut h = History::new();
        h.begin_batch();
        h.record(op(0, "", "h"));
        h.record(op(1, "", "i"));
        h.commit_batch();
        assert!(h.can_undo());
        let b = h.pop_undo().unwrap();
        assert_eq!(b.ops.len(), 2);
        assert!(h.can_redo());
        let r = h.pop_redo().unwrap();
        assert_eq!(r.ops.len(), 2);
    }

    #[test]
    fn record_without_batch_pushes_individual() {
        let mut h = History::new();
        h.record(op(0, "", "x"));
        h.record(op(1, "", "y"));
        assert!(h.pop_undo().is_some());
        assert!(h.pop_undo().is_some());
        assert!(!h.can_undo());
    }
}
