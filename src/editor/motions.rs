//! Pure cursor/motion helpers operating on a [`Buffer`].
//!
//! These functions take and return positions; they never mutate the rope.

use crate::editor::buffer::{Buffer, Point};
use crate::editor::Cursor;

/// Character classification for word motions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass { Word, Punct, Space, Newline }

fn classify(c: char) -> CharClass {
    if c == '\n' { CharClass::Newline }
    else if c.is_whitespace() { CharClass::Space }
    else if c.is_alphanumeric() || c == '_' { CharClass::Word }
    else { CharClass::Punct }
}

fn clamp_col(buf: &Buffer, row: usize, col: usize) -> usize {
    let max = buf.line_len_chars(row);
    col.min(max.saturating_sub(if max == 0 { 0 } else { 1 }))
}

/// Move left within line.
pub fn left(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let col = cur.col.saturating_sub(n);
    Cursor::new(cur.row, col)
}

/// Move right within line, clamped to last char.
pub fn right(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let max = buf.line_len_chars(cur.row).saturating_sub(1);
    let col = (cur.col + n).min(max);
    Cursor::new(cur.row, col)
}

pub fn down(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let row = (cur.row + n).min(buf.line_count().saturating_sub(1));
    let max = buf.line_len_chars(row).saturating_sub(1);
    let col = cur.want_col.min(max);
    Cursor { row, col, want_col: cur.want_col }
}

pub fn up(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let row = cur.row.saturating_sub(n);
    let max = buf.line_len_chars(row).saturating_sub(1);
    let col = cur.want_col.min(max);
    Cursor { row, col, want_col: cur.want_col }
}

pub fn line_start(_buf: &Buffer, cur: Cursor) -> Cursor {
    Cursor::new(cur.row, 0)
}

pub fn line_first_non_blank(buf: &Buffer, cur: Cursor) -> Cursor {
    let s = buf.line(cur.row).to_string();
    let col = s.chars().take_while(|c| c.is_whitespace() && *c != '\n').count();
    Cursor::new(cur.row, col)
}

pub fn line_end(buf: &Buffer, cur: Cursor) -> Cursor {
    let max = buf.line_len_chars(cur.row).saturating_sub(1);
    let col = max.max(0);
    let mut c = Cursor::new(cur.row, col);
    c.want_col = usize::MAX / 2;
    c
}

pub fn buffer_top(_buf: &Buffer, _cur: Cursor) -> Cursor { Cursor::new(0, 0) }

pub fn buffer_bottom(buf: &Buffer, _cur: Cursor) -> Cursor {
    let row = buf.line_count().saturating_sub(1);
    Cursor::new(row, 0)
}

pub fn goto_line(buf: &Buffer, _cur: Cursor, line_1based: usize) -> Cursor {
    let row = line_1based.saturating_sub(1).min(buf.line_count().saturating_sub(1));
    Cursor::new(row, 0)
}

/// Iterate (row, col, char) forwards from a starting point.
fn iter_forward(buf: &Buffer, start: Cursor) -> Vec<(usize, usize, char)> {
    let mut out = Vec::new();
    for row in start.row..buf.line_count() {
        let line = buf.line(row).to_string();
        let chars: Vec<char> = line.chars().collect();
        let start_col = if row == start.row { start.col } else { 0 };
        for (c_idx, &ch) in chars.iter().enumerate().skip(start_col) {
            out.push((row, c_idx, ch));
        }
    }
    out
}

/// Iterate backwards (inclusive of start).
fn iter_backward(buf: &Buffer, start: Cursor) -> Vec<(usize, usize, char)> {
    let mut out = Vec::new();
    for row in (0..=start.row).rev() {
        let line = buf.line(row).to_string();
        let chars: Vec<char> = line.chars().collect();
        let end = if row == start.row { (start.col + 1).min(chars.len()) } else { chars.len() };
        for c_idx in (0..end).rev() {
            out.push((row, c_idx, chars[c_idx]));
        }
    }
    out
}

/// `w` — start of next word.
pub fn word_forward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut pos = cur;
    for _ in 0..n {
        let seq = iter_forward(buf, pos);
        if seq.len() < 2 { break; }
        let start_class = classify(seq[0].2);
        let mut found = None;
        for w in seq.windows(2) {
            let (_, _, a) = w[0];
            let (br, bc, b) = w[1];
            let ca = classify(a);
            let cb = classify(b);
            if cb != CharClass::Space && cb != CharClass::Newline && ca != cb {
                found = Some((br, bc));
                break;
            }
        }
        if let Some((r, c)) = found { pos = Cursor::new(r, c); } else { break; }
    }
    pos
}

/// `b` — start of previous word.
pub fn word_backward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut pos = cur;
    for _ in 0..n {
        let seq = iter_backward(buf, pos);
        if seq.len() < 2 { break; }
        // skip leading whitespace
        let mut i = 1;
        while i < seq.len() && (classify(seq[i].2) == CharClass::Space || classify(seq[i].2) == CharClass::Newline) {
            i += 1;
        }
        if i >= seq.len() { break; }
        let cls = classify(seq[i].2);
        // walk back while same class
        while i + 1 < seq.len() && classify(seq[i + 1].2) == cls {
            i += 1;
        }
        let (r, c, _) = seq[i];
        pos = Cursor::new(r, c);
    }
    pos
}

/// `e` — end of word.
pub fn word_end(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut pos = cur;
    for _ in 0..n {
        let seq = iter_forward(buf, pos);
        if seq.len() < 2 { break; }
        let mut i = 1;
        // skip whitespace
        while i < seq.len() && (classify(seq[i].2) == CharClass::Space || classify(seq[i].2) == CharClass::Newline) {
            i += 1;
        }
        if i >= seq.len() { break; }
        let cls = classify(seq[i].2);
        while i + 1 < seq.len() && classify(seq[i + 1].2) == cls {
            i += 1;
        }
        let (r, c, _) = seq[i];
        pos = Cursor::new(r, c);
    }
    pos
}

/// `W` — WORD-forward (whitespace-separated).
pub fn WORD_forward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut pos = cur;
    for _ in 0..n {
        let seq = iter_forward(buf, pos);
        if seq.len() < 2 { break; }
        let mut i = 0;
        // walk to whitespace
        while i + 1 < seq.len() && classify(seq[i].2) != CharClass::Space && classify(seq[i].2) != CharClass::Newline { i += 1; }
        // walk past whitespace
        while i < seq.len() && (classify(seq[i].2) == CharClass::Space || classify(seq[i].2) == CharClass::Newline) { i += 1; }
        if i >= seq.len() { break; }
        let (r, c, _) = seq[i];
        pos = Cursor::new(r, c);
    }
    pos
}

pub fn WORD_backward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut pos = cur;
    for _ in 0..n {
        let seq = iter_backward(buf, pos);
        if seq.len() < 2 { break; }
        let mut i = 1;
        while i < seq.len() && (classify(seq[i].2) == CharClass::Space || classify(seq[i].2) == CharClass::Newline) { i += 1; }
        while i + 1 < seq.len() && classify(seq[i + 1].2) != CharClass::Space && classify(seq[i + 1].2) != CharClass::Newline { i += 1; }
        if i >= seq.len() { break; }
        let (r, c, _) = seq[i];
        pos = Cursor::new(r, c);
    }
    pos
}

pub fn WORD_end(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut pos = cur;
    for _ in 0..n {
        let seq = iter_forward(buf, pos);
        if seq.len() < 2 { break; }
        let mut i = 1;
        while i < seq.len() && (classify(seq[i].2) == CharClass::Space || classify(seq[i].2) == CharClass::Newline) { i += 1; }
        while i + 1 < seq.len() && classify(seq[i + 1].2) != CharClass::Space && classify(seq[i + 1].2) != CharClass::Newline { i += 1; }
        if i >= seq.len() { break; }
        let (r, c, _) = seq[i];
        pos = Cursor::new(r, c);
    }
    pos
}

/// `f`/`F`/`t`/`T` — find char on current line.
pub fn find_char(buf: &Buffer, cur: Cursor, ch: char, forward: bool, till: bool, n: usize) -> Cursor {
    let line = buf.line(cur.row).to_string();
    let chars: Vec<char> = line.trim_end_matches('\n').chars().collect();
    let mut col = cur.col;
    let mut hits = 0;
    if forward {
        let mut i = col + 1;
        while i < chars.len() {
            if chars[i] == ch {
                hits += 1;
                if hits == n {
                    col = if till { i.saturating_sub(1) } else { i };
                    break;
                }
            }
            i += 1;
        }
    } else {
        let mut i = col;
        while i > 0 {
            i -= 1;
            if chars[i] == ch {
                hits += 1;
                if hits == n {
                    col = if till { i + 1 } else { i };
                    break;
                }
            }
        }
    }
    Cursor::new(cur.row, col)
}

/// `%` — match nearest bracket on current line; falls back to current pos.
pub fn match_bracket(buf: &Buffer, cur: Cursor) -> Cursor {
    let pairs: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];
    let line = buf.line(cur.row).to_string();
    let chars: Vec<char> = line.chars().collect();
    if cur.col >= chars.len() { return cur; }
    let here = chars[cur.col];
    for &(o, c) in pairs {
        if here == o { return scan_match(buf, cur, o, c, true); }
        if here == c { return scan_match(buf, cur, c, o, false); }
    }
    cur
}

fn scan_match(buf: &Buffer, start: Cursor, open: char, close: char, forward: bool) -> Cursor {
    let mut depth = 0i32;
    let seq = if forward { iter_forward(buf, start) } else { iter_backward(buf, start) };
    for (r, c, ch) in seq {
        if ch == open { depth += 1; }
        else if ch == close {
            depth -= 1;
            if depth == 0 { return Cursor::new(r, c); }
        }
    }
    start
}

/// Paragraph forward: next blank line.
pub fn paragraph_forward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut row = cur.row;
    for _ in 0..n {
        let mut r = row + 1;
        // skip blank lines
        while r < buf.line_count() && buf.line_len_chars(r) == 0 { r += 1; }
        // walk to next blank
        while r < buf.line_count() && buf.line_len_chars(r) > 0 { r += 1; }
        row = r.min(buf.line_count().saturating_sub(1));
    }
    Cursor::new(row, 0)
}

pub fn paragraph_backward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    let mut row = cur.row;
    for _ in 0..n {
        if row == 0 { break; }
        let mut r = row - 1;
        while r > 0 && buf.line_len_chars(r) == 0 { r -= 1; }
        while r > 0 && buf.line_len_chars(r) > 0 { r -= 1; }
        row = r;
    }
    Cursor::new(row, 0)
}

pub fn sentence_forward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    // Approximation: same as paragraph for the moment.
    paragraph_forward(buf, cur, n)
}

pub fn sentence_backward(buf: &Buffer, cur: Cursor, n: usize) -> Cursor {
    paragraph_backward(buf, cur, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Buffer;

    fn buf(s: &str) -> Buffer { Buffer::from_str(s) }

    #[test]
    fn h_l_basic() {
        let b = buf("hello");
        let c = Cursor::new(0, 2);
        assert_eq!(left(&b, c, 1).col, 1);
        assert_eq!(right(&b, c, 1).col, 3);
        assert_eq!(right(&b, c, 99).col, 4);
        assert_eq!(left(&b, c, 99).col, 0);
    }

    #[test]
    fn j_k_with_want() {
        let b = buf("hello\nhi\nworld");
        let c = Cursor { row: 0, col: 4, want_col: 4 };
        let d = down(&b, c, 1);
        assert_eq!((d.row, d.col), (1, 1)); // clamped to len-1
        let dd = down(&b, d, 1);
        assert_eq!((dd.row, dd.col), (2, 4)); // restored
    }

    #[test]
    fn line_start_end() {
        let b = buf("  hello");
        let c = Cursor::new(0, 5);
        assert_eq!(line_start(&b, c).col, 0);
        assert_eq!(line_first_non_blank(&b, c).col, 2);
        assert_eq!(line_end(&b, c).col, 6);
    }

    #[test]
    fn buffer_top_bottom() {
        let b = buf("a\nb\nc");
        assert_eq!(buffer_top(&b, Cursor::default()).row, 0);
        assert_eq!(buffer_bottom(&b, Cursor::default()).row, 2);
    }

    #[test]
    fn word_motion_basic() {
        let b = buf("foo bar baz");
        let c = Cursor::new(0, 0);
        let w1 = word_forward(&b, c, 1);
        assert_eq!(w1.col, 4);
        let w2 = word_forward(&b, c, 2);
        assert_eq!(w2.col, 8);
        let bw = word_backward(&b, w2, 1);
        assert_eq!(bw.col, 4);
        let we = word_end(&b, c, 1);
        assert_eq!(we.col, 2);
    }

    #[test]
    fn find_char_works() {
        let b = buf("hello world");
        let c = Cursor::new(0, 0);
        assert_eq!(find_char(&b, c, 'o', true, false, 1).col, 4);
        assert_eq!(find_char(&b, c, 'o', true, false, 2).col, 7);
        assert_eq!(find_char(&b, c, 'o', true, true, 1).col, 3);
    }

    #[test]
    fn match_bracket_pair() {
        let b = buf("(a(b)c)");
        let c = Cursor::new(0, 0);
        let m = match_bracket(&b, c);
        assert_eq!(m.col, 6);
    }

    #[test]
    fn paragraph_jumps() {
        let b = buf("a\nb\n\nc\nd\n\ne");
        let c = Cursor::new(0, 0);
        let p = paragraph_forward(&b, c, 1);
        assert!(p.row >= 2);
    }
}
