//! Text buffer backed by a ropey rope.

use crate::app::BufferId;
use crate::editor::history::{EditOp, History};
use crate::editor::{Marks, Registers};
use anyhow::Result;
use ropey::Rope;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// (row, col) byte-column point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

/// A single text mutation, in tree-sitter's `InputEdit` shape (byte
/// offsets + byte-columned row/col triples). Produced by `Buffer` on
/// every `insert` / `delete` / `apply_raw` and drained by
/// `HighlightEngine` for incremental parsing.
#[derive(Debug, Clone)]
pub struct BufferEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_row: usize,
    pub start_col: usize,
    pub old_end_row: usize,
    pub old_end_col: usize,
    pub new_end_row: usize,
    pub new_end_col: usize,
}

fn byte_point(rope: &Rope, byte: usize) -> (usize, usize) {
    let byte = byte.min(rope.len_bytes());
    let row = rope.byte_to_line(byte);
    let col = byte - rope.line_to_byte(row);
    (row, col)
}

#[derive(Debug, Default)]
pub struct Buffer {
    pub id: BufferId,
    pub rope: Rope,
    pub path: Option<PathBuf>,
    pub language_id: Option<String>,
    pub dirty: bool,
    pub history: History,
    pub registers: Registers,
    pub marks: Marks,
    pub diagnostics: Vec<lsp_types::Diagnostic>,
    pub version: i32,
    pub pending_edits: Vec<BufferEdit>,
    /// LSP document URI once a server has opened this buffer.
    pub lsp_uri: Option<lsp_types::Uri>,
    /// Set by `insert`/`delete`/`apply_raw` — drained by the app loop to
    /// send a `textDocument/didChange` to the language server.
    pub lsp_dirty: bool,
}

impl Buffer {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            ..Self::default()
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // Canonical language id via the grammar registry (e.g. "rs" -> "rust").
        // Falls back to the raw file extension if the registry has no entry.
        let lang = crate::highlight::grammar_registry::GrammarRegistry::global()
            .for_path(&path)
            .map(|e| e.id.to_string())
            .or_else(|| {
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_string())
            });
        Ok(Self {
            rope: Rope::from_str(&text),
            path: Some(path),
            language_id: lang,
            ..Self::default()
        })
    }

    pub fn save(&mut self) -> Result<()> {
        if let Some(p) = &self.path {
            let text = self.rope.to_string();
            std::fs::write(p, text)?;
            self.dirty = false;
        }
        Ok(())
    }

    pub fn line_count(&self) -> usize {
        // ropey reports an empty trailing line for trailing newline; vim treats it as N lines.
        self.rope.len_lines().max(1)
    }

    pub fn line(&self, idx: usize) -> ropey::RopeSlice<'_> {
        self.rope.line(idx)
    }

    /// Length of `line` in bytes excluding the trailing newline.
    pub fn line_len_bytes(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let l = self.rope.line(line);
        let mut n = l.len_bytes();
        // strip trailing \n / \r\n
        if n > 0 {
            let s = l.to_string();
            if s.ends_with('\n') {
                n -= 1;
            }
            if s.ends_with("\r\n") {
                n -= 1;
            }
        }
        n
    }

    pub fn line_len_chars(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let l = self.rope.line(line);
        let s = l.to_string();
        let trimmed = s.trim_end_matches('\n').trim_end_matches('\r');
        trimmed.chars().count()
    }

    /// Convert a byte offset to (row, col) where col is char-column within line.
    pub fn byte_to_point(&self, byte: usize) -> Point {
        let byte = byte.min(self.rope.len_bytes());
        let char_idx = self.rope.byte_to_char(byte);
        let row = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(row);
        let col = char_idx - line_start;
        Point { row, col }
    }

    /// Convert (row, col) char-col to byte offset.
    pub fn point_to_byte(&self, p: Point) -> usize {
        let row = p.row.min(self.line_count().saturating_sub(1));
        let line_char_start = self.rope.line_to_char(row);
        let line_chars = self.line_len_chars(row);
        let col = p.col.min(line_chars);
        let char_idx = line_char_start + col;
        self.rope.char_to_byte(char_idx)
    }

    /// Insert at byte offset. Records to history.
    pub fn insert(&mut self, byte_pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let (start_row, start_col) = byte_point(&self.rope, byte_pos);
        let char_idx = self.rope.byte_to_char(byte_pos);
        self.rope.insert(char_idx, text);
        let new_end_byte = byte_pos + text.len();
        let (new_end_row, new_end_col) = byte_point(&self.rope, new_end_byte);
        self.dirty = true;
        self.lsp_dirty = true;
        self.version += 1;
        self.pending_edits.push(BufferEdit {
            start_byte: byte_pos,
            old_end_byte: byte_pos,
            new_end_byte,
            start_row,
            start_col,
            old_end_row: start_row,
            old_end_col: start_col,
            new_end_row,
            new_end_col,
        });
        self.history.record(EditOp {
            start: byte_pos,
            removed: String::new(),
            inserted: text.to_string(),
            cursor_before: byte_pos,
            cursor_after: new_end_byte,
        });
    }

    /// Delete a byte range. Records to history. Returns deleted text.
    pub fn delete(&mut self, range: Range<usize>) -> String {
        if range.start >= range.end {
            return String::new();
        }
        let (start_row, start_col) = byte_point(&self.rope, range.start);
        let (old_end_row, old_end_col) = byte_point(&self.rope, range.end);
        let start_c = self.rope.byte_to_char(range.start);
        let end_c = self.rope.byte_to_char(range.end);
        let removed: String = self.rope.slice(start_c..end_c).to_string();
        self.rope.remove(start_c..end_c);
        self.dirty = true;
        self.lsp_dirty = true;
        self.version += 1;
        self.pending_edits.push(BufferEdit {
            start_byte: range.start,
            old_end_byte: range.end,
            new_end_byte: range.start,
            start_row,
            start_col,
            old_end_row,
            old_end_col,
            new_end_row: start_row,
            new_end_col: start_col,
        });
        self.history.record(EditOp {
            start: range.start,
            removed: removed.clone(),
            inserted: String::new(),
            cursor_before: range.end,
            cursor_after: range.start,
        });
        removed
    }

    /// Apply an edit op (used by undo/redo) WITHOUT recording history,
    /// but still emitting a `BufferEdit` so the parse tree stays in sync.
    pub(crate) fn apply_raw(&mut self, start: usize, removed_len: usize, inserted: &str) {
        let old_end_byte = start + removed_len;
        let (start_row, start_col) = byte_point(&self.rope, start);
        let (old_end_row, old_end_col) = byte_point(&self.rope, old_end_byte);
        let start_c = self.rope.byte_to_char(start);
        if removed_len > 0 {
            let end_c = self.rope.byte_to_char(old_end_byte);
            self.rope.remove(start_c..end_c);
        }
        if !inserted.is_empty() {
            self.rope.insert(start_c, inserted);
        }
        let new_end_byte = start + inserted.len();
        let (new_end_row, new_end_col) = byte_point(&self.rope, new_end_byte);
        self.dirty = true;
        self.lsp_dirty = true;
        self.version += 1;
        self.pending_edits.push(BufferEdit {
            start_byte: start,
            old_end_byte,
            new_end_byte,
            start_row,
            start_col,
            old_end_row,
            old_end_col,
            new_end_row,
            new_end_col,
        });
    }

    /// Undo the most recent batch. Returns the cursor byte position to restore.
    pub fn undo(&mut self) -> Option<usize> {
        let batch = self.history.pop_undo()?;
        let mut cursor = 0usize;
        // Reverse-apply ops in reverse order.
        for op in batch.ops.iter().rev() {
            self.apply_raw(op.start, op.inserted.len(), &op.removed);
            cursor = op.cursor_before;
        }
        Some(cursor)
    }

    pub fn redo(&mut self) -> Option<usize> {
        let batch = self.history.pop_redo()?;
        let mut cursor = 0usize;
        for op in &batch.ops {
            self.apply_raw(op.start, op.removed.len(), &op.inserted);
            cursor = op.cursor_after;
        }
        Some(cursor)
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn slice_bytes(&self, range: Range<usize>) -> String {
        let start_c = self
            .rope
            .byte_to_char(range.start.min(self.rope.len_bytes()));
        let end_c = self.rope.byte_to_char(range.end.min(self.rope.len_bytes()));
        self.rope.slice(start_c..end_c).to_string()
    }

    pub fn char_at_byte(&self, byte: usize) -> Option<char> {
        if byte >= self.rope.len_bytes() {
            return None;
        }
        let c = self.rope.byte_to_char(byte);
        self.rope.get_char(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_and_lines() {
        let b = Buffer::from_str("hello\nworld\n");
        assert!(b.line_count() >= 2);
        assert_eq!(b.line_len_chars(0), 5);
        assert_eq!(b.line_len_chars(1), 5);
    }

    #[test]
    fn point_byte_roundtrip() {
        let b = Buffer::from_str("abc\ndef\nghi");
        let bp = b.point_to_byte(Point { row: 1, col: 2 });
        let p = b.byte_to_point(bp);
        assert_eq!(p, Point { row: 1, col: 2 });
    }

    #[test]
    fn insert_delete_dirty() {
        let mut b = Buffer::from_str("abc");
        b.insert(1, "X");
        assert_eq!(b.rope.to_string(), "aXbc");
        assert!(b.dirty);
        b.delete(1..2);
        assert_eq!(b.rope.to_string(), "abc");
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut b = Buffer::from_str("hello");
        b.insert(5, " world");
        assert_eq!(b.rope.to_string(), "hello world");
        b.undo();
        assert_eq!(b.rope.to_string(), "hello");
        b.redo();
        assert_eq!(b.rope.to_string(), "hello world");
    }

    #[test]
    fn batched_undo() {
        let mut b = Buffer::from_str("");
        b.history.begin_batch();
        b.insert(0, "h");
        b.insert(1, "i");
        b.history.commit_batch();
        b.undo();
        assert_eq!(b.rope.to_string(), "");
    }
}
