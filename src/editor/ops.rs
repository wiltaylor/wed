//! Operators acting on byte ranges of a [`Buffer`].

use crate::editor::buffer::Buffer;
use crate::editor::registers::{RegisterEntry, Registers, YankKind};
use crate::editor::text_objects::ByteRange;
use crate::editor::Cursor;

pub fn yank_range(buf: &Buffer, regs: &mut Registers, range: ByteRange, kind: YankKind) -> String {
    let text = buf.slice_bytes(range.start..range.end);
    regs.set_unnamed(text.clone(), kind);
    text
}

pub fn delete_range(
    buf: &mut Buffer,
    regs: &mut Registers,
    range: ByteRange,
    kind: YankKind,
) -> String {
    let text = buf.slice_bytes(range.start..range.end);
    regs.set_unnamed(text.clone(), kind);
    buf.delete(range.start..range.end);
    text
}

/// Paste after cursor; returns new cursor byte position.
pub fn paste_after(buf: &mut Buffer, cursor_byte: usize, entry: &RegisterEntry) -> usize {
    if entry.text.is_empty() {
        return cursor_byte;
    }
    match entry.kind {
        YankKind::Line => {
            // Paste on next line.
            let p = buf.byte_to_point(cursor_byte);
            let next_line_start_char = if p.row + 1 < buf.line_count() {
                buf.rope.line_to_char(p.row + 1)
            } else {
                buf.rope.len_chars()
            };
            let next_line_byte = buf.rope.char_to_byte(next_line_start_char);
            let mut text = entry.text.clone();
            if !text.ends_with('\n') {
                text.push('\n');
            }
            buf.insert(next_line_byte, &text);
            next_line_byte
        }
        _ => {
            let pos = (cursor_byte + 1).min(buf.len_bytes());
            buf.insert(pos, &entry.text);
            pos + entry.text.len() - 1
        }
    }
}

pub fn paste_before(buf: &mut Buffer, cursor_byte: usize, entry: &RegisterEntry) -> usize {
    if entry.text.is_empty() {
        return cursor_byte;
    }
    match entry.kind {
        YankKind::Line => {
            let p = buf.byte_to_point(cursor_byte);
            let line_start_char = buf.rope.line_to_char(p.row);
            let line_start_byte = buf.rope.char_to_byte(line_start_char);
            let mut text = entry.text.clone();
            if !text.ends_with('\n') {
                text.push('\n');
            }
            buf.insert(line_start_byte, &text);
            line_start_byte
        }
        _ => {
            buf.insert(cursor_byte, &entry.text);
            cursor_byte + entry.text.len() - 1
        }
    }
}

/// Indent the rows covered by `range_rows` by inserting a prefix.
pub fn indent_rows(buf: &mut Buffer, rows: std::ops::RangeInclusive<usize>, prefix: &str) {
    for row in rows.clone() {
        let line_char = buf.rope.line_to_char(row);
        let line_byte = buf.rope.char_to_byte(line_char);
        buf.insert(line_byte, prefix);
    }
}

/// Dedent: strip up to `prefix.len()` of leading whitespace from each row.
pub fn dedent_rows(buf: &mut Buffer, rows: std::ops::RangeInclusive<usize>, prefix_len: usize) {
    for row in rows {
        let line_char = buf.rope.line_to_char(row);
        let line_byte = buf.rope.char_to_byte(line_char);
        let line = buf.line(row).to_string();
        let to_remove = line
            .chars()
            .take(prefix_len)
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        if to_remove > 0 {
            buf.delete(line_byte..line_byte + to_remove);
        }
    }
}

/// Toggle a line comment for the given rows using `comment_str`.
pub fn comment_toggle_rows(
    buf: &mut Buffer,
    rows: std::ops::RangeInclusive<usize>,
    comment_str: &str,
) {
    // If every non-blank line starts with comment_str, remove it; else add it.
    let prefix = format!("{} ", comment_str);
    let mut all_commented = true;
    let mut any_nonblank = false;
    for row in rows.clone() {
        let line = buf.line(row).to_string();
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        any_nonblank = true;
        if !trimmed.starts_with(comment_str) {
            all_commented = false;
            break;
        }
    }
    let action_uncomment = all_commented && any_nonblank;
    for row in rows {
        let line = buf.line(row).to_string();
        if line.trim().is_empty() {
            continue;
        }
        let line_char = buf.rope.line_to_char(row);
        let line_byte = buf.rope.char_to_byte(line_char);
        let leading = line
            .chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .count();
        let content_byte = line_byte + leading;
        if action_uncomment {
            // strip comment_str (and a following space if present)
            let after = &line[leading..];
            let strip = if after.starts_with(&prefix) {
                prefix.len()
            } else {
                comment_str.len()
            };
            buf.delete(content_byte..content_byte + strip);
        } else {
            buf.insert(content_byte, &prefix);
        }
    }
}

/// Returns line-comment string for a language id (best-effort).
pub fn comment_string_for(language_id: Option<&str>) -> &'static str {
    match language_id {
        Some(
            "rs" | "go" | "c" | "cpp" | "cc" | "h" | "hpp" | "js" | "ts" | "tsx" | "jsx" | "java"
            | "swift" | "kt",
        ) => "//",
        Some("py" | "sh" | "bash" | "zsh" | "rb" | "yaml" | "yml" | "toml" | "conf") => "#",
        Some("lua" | "sql" | "hs") => "--",
        Some("vim") => "\"",
        _ => "//",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::registers::{Registers, YankKind};
    use crate::editor::text_objects::inner_word;

    #[test]
    fn delete_word_via_text_object() {
        let mut b = Buffer::from_str("hello world");
        let mut r = Registers::new();
        let range = inner_word(&b, Cursor::new(0, 0)).unwrap();
        delete_range(&mut b, &mut r, range, YankKind::Char);
        assert_eq!(b.rope.to_string(), " world");
        assert_eq!(r.get('"').unwrap().text, "hello");
    }

    #[test]
    fn paste_after_char() {
        let mut b = Buffer::from_str("ac");
        let entry = RegisterEntry {
            text: "b".into(),
            kind: YankKind::Char,
        };
        let pos = paste_after(&mut b, 0, &entry);
        assert_eq!(b.rope.to_string(), "abc");
        assert_eq!(pos, 1);
    }

    #[test]
    fn paste_line_inserts_below() {
        let mut b = Buffer::from_str("a\nc\n");
        let entry = RegisterEntry {
            text: "b".into(),
            kind: YankKind::Line,
        };
        paste_after(&mut b, 0, &entry);
        assert_eq!(b.rope.to_string(), "a\nb\nc\n");
    }

    #[test]
    fn comment_toggle_rs() {
        let mut b = Buffer::from_str("foo\nbar\n");
        comment_toggle_rows(&mut b, 0..=1, "//");
        assert!(b.rope.to_string().starts_with("// foo"));
        comment_toggle_rows(&mut b, 0..=1, "//");
        assert_eq!(b.rope.to_string(), "foo\nbar\n");
    }

    #[test]
    fn indent_dedent() {
        let mut b = Buffer::from_str("a\nb\n");
        indent_rows(&mut b, 0..=1, "    ");
        assert_eq!(b.rope.to_string(), "    a\n    b\n");
        dedent_rows(&mut b, 0..=1, 4);
        assert_eq!(b.rope.to_string(), "a\nb\n");
    }
}
