//! Vim text-object range computation.

use crate::editor::buffer::Buffer;
use crate::editor::Cursor;

/// A char-range within the buffer expressed as (start_byte, end_byte_exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

fn point_to_byte(buf: &Buffer, row: usize, col: usize) -> usize {
    buf.point_to_byte(crate::editor::buffer::Point { row, col })
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `iw` — inner word.
pub fn inner_word(buf: &Buffer, cur: Cursor) -> Option<ByteRange> {
    let line = buf.line(cur.row).to_string();
    let chars: Vec<char> = line.trim_end_matches('\n').chars().collect();
    if chars.is_empty() || cur.col >= chars.len() {
        return None;
    }
    let mut s = cur.col;
    let mut e = cur.col;
    let pred: fn(char) -> bool = if is_word(chars[cur.col]) {
        |c| is_word(c)
    } else if chars[cur.col].is_whitespace() {
        |c| c.is_whitespace()
    } else {
        |c| !is_word(c) && !c.is_whitespace()
    };
    while s > 0 && pred(chars[s - 1]) {
        s -= 1;
    }
    while e + 1 < chars.len() && pred(chars[e + 1]) {
        e += 1;
    }
    Some(ByteRange {
        start: point_to_byte(buf, cur.row, s),
        end: point_to_byte(buf, cur.row, e + 1),
    })
}

pub fn around_word(buf: &Buffer, cur: Cursor) -> Option<ByteRange> {
    let mut r = inner_word(buf, cur)?;
    // include trailing whitespace
    let line = buf.line(cur.row).to_string();
    let chars: Vec<char> = line.trim_end_matches('\n').chars().collect();
    let end_p = buf.byte_to_point(r.end);
    let mut e = end_p.col;
    while e < chars.len() && chars[e].is_whitespace() {
        e += 1;
    }
    r.end = point_to_byte(buf, cur.row, e);
    Some(r)
}

/// `i<delim>` — inside delimiter pair on current line.
pub fn inner_pair(buf: &Buffer, cur: Cursor, open: char, close: char) -> Option<ByteRange> {
    let line = buf.line(cur.row).to_string();
    let chars: Vec<char> = line.trim_end_matches('\n').chars().collect();
    if chars.is_empty() {
        return None;
    }
    // search left for `open`
    let mut s = None;
    let mut depth = 0i32;
    let start = cur.col.min(chars.len().saturating_sub(1));
    for i in (0..=start).rev() {
        if chars[i] == close && open != close {
            depth += 1;
        } else if chars[i] == open {
            if depth == 0 {
                s = Some(i);
                break;
            }
            depth -= 1;
        }
    }
    let s = s?;
    let mut e = None;
    let mut depth = 0i32;
    for (i, ch) in chars.iter().enumerate().skip(s + 1) {
        if *ch == open && open != close {
            depth += 1;
        } else if *ch == close {
            if depth == 0 {
                e = Some(i);
                break;
            }
            depth -= 1;
        }
    }
    let e = e?;
    Some(ByteRange {
        start: point_to_byte(buf, cur.row, s + 1),
        end: point_to_byte(buf, cur.row, e),
    })
}

pub fn around_pair(buf: &Buffer, cur: Cursor, open: char, close: char) -> Option<ByteRange> {
    let mut r = inner_pair(buf, cur, open, close)?;
    r.start = r.start.saturating_sub(1);
    r.end += 1;
    Some(r)
}

/// `ip` — inner paragraph.
pub fn inner_paragraph(buf: &Buffer, cur: Cursor) -> Option<ByteRange> {
    let mut s = cur.row;
    let mut e = cur.row;
    while s > 0 && buf.line_len_chars(s - 1) > 0 {
        s -= 1;
    }
    while e + 1 < buf.line_count() && buf.line_len_chars(e + 1) > 0 {
        e += 1;
    }
    let start = point_to_byte(buf, s, 0);
    let end_col = buf.line_len_chars(e);
    let end = point_to_byte(buf, e, end_col);
    Some(ByteRange { start, end })
}

pub fn around_paragraph(buf: &Buffer, cur: Cursor) -> Option<ByteRange> {
    let mut r = inner_paragraph(buf, cur)?;
    let p = buf.byte_to_point(r.end);
    let mut e = p.row;
    while e + 1 < buf.line_count() && buf.line_len_chars(e + 1) == 0 {
        e += 1;
    }
    r.end = point_to_byte(buf, e, buf.line_len_chars(e));
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Buffer;

    #[test]
    fn iw_basic() {
        let b = Buffer::from_str("hello world");
        let r = inner_word(&b, Cursor::new(0, 7)).unwrap();
        assert_eq!(b.slice_bytes(r.start..r.end), "world");
    }

    #[test]
    fn aw_includes_space() {
        let b = Buffer::from_str("hello world");
        let r = around_word(&b, Cursor::new(0, 1)).unwrap();
        assert_eq!(b.slice_bytes(r.start..r.end), "hello ");
    }

    #[test]
    fn inner_quotes() {
        let b = Buffer::from_str("foo \"bar baz\" qux");
        let r = inner_pair(&b, Cursor::new(0, 6), '"', '"').unwrap();
        assert_eq!(b.slice_bytes(r.start..r.end), "bar baz");
    }

    #[test]
    fn around_parens() {
        let b = Buffer::from_str("call(a, b)");
        let r = around_pair(&b, Cursor::new(0, 6), '(', ')').unwrap();
        assert_eq!(b.slice_bytes(r.start..r.end), "(a, b)");
    }

    #[test]
    fn paragraph_object() {
        let b = Buffer::from_str("a\nb\n\nc\nd");
        let r = inner_paragraph(&b, Cursor::new(0, 0)).unwrap();
        assert_eq!(b.slice_bytes(r.start..r.end), "a\nb");
    }
}
