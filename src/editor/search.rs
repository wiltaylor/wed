//! Regex-based search.

use crate::editor::buffer::Buffer;
use crate::editor::Cursor;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct SearchState {
    pub pattern: String,
    pub regex: Option<Regex>,
    pub forward: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            regex: None,
            forward: true,
        }
    }
}

impl SearchState {
    pub fn set(&mut self, pattern: &str, forward: bool) {
        self.pattern = pattern.to_string();
        self.regex = Regex::new(pattern).ok();
        self.forward = forward;
    }
}

/// Find next match starting after `cur`. Returns the match cursor.
pub fn search_next(buf: &Buffer, state: &SearchState, cur: Cursor) -> Option<Cursor> {
    let re = state.regex.as_ref()?;
    let text = buf.rope.to_string();
    let start_byte = buf.point_to_byte(crate::editor::buffer::Point {
        row: cur.row,
        col: cur.col,
    });
    let after = (start_byte + 1).min(text.len());
    if let Some(m) = re.find_at(&text, after) {
        let p = buf.byte_to_point(m.start());
        return Some(Cursor::new(p.row, p.col));
    }
    // wrap
    if let Some(m) = re.find(&text) {
        let p = buf.byte_to_point(m.start());
        return Some(Cursor::new(p.row, p.col));
    }
    None
}

/// Find previous match strictly before `cur`.
pub fn search_prev(buf: &Buffer, state: &SearchState, cur: Cursor) -> Option<Cursor> {
    let re = state.regex.as_ref()?;
    let text = buf.rope.to_string();
    let here_byte = buf.point_to_byte(crate::editor::buffer::Point {
        row: cur.row,
        col: cur.col,
    });
    let mut last = None;
    for m in re.find_iter(&text) {
        if m.start() < here_byte {
            last = Some(m.start());
        } else {
            break;
        }
    }
    let pos = last.or_else(|| re.find_iter(&text).last().map(|m| m.start()))?;
    let p = buf.byte_to_point(pos);
    Some(Cursor::new(p.row, p.col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Buffer;

    #[test]
    fn next_and_prev() {
        let b = Buffer::from_str("foo bar foo baz");
        let mut s = SearchState::default();
        s.set("foo", true);
        let n1 = search_next(&b, &s, Cursor::new(0, 0)).unwrap();
        assert_eq!(n1.col, 8);
        let n2 = search_next(&b, &s, n1).unwrap();
        // wraps to 0
        assert_eq!(n2.col, 0);
        let p = search_prev(&b, &s, Cursor::new(0, 9)).unwrap();
        assert_eq!(p.col, 8);
    }
}
