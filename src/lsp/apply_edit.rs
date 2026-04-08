//! Apply an LSP `WorkspaceEdit` to open buffers.
//!
//! Converts LSP (line, utf16_char) positions to byte offsets via the same
//! walk used by the diagnostic renderer, then applies edits in reverse byte
//! order per file so earlier offsets stay stable.

use lsp_types::{DocumentChanges, OneOf, Position, TextEdit, Uri, WorkspaceEdit};

use crate::app::App;

/// (line, utf16_col) → absolute byte offset in `rope`.
pub fn lsp_pos_to_byte(rope: &ropey::Rope, pos: Position) -> Option<usize> {
    let line = pos.line as usize;
    if line >= rope.len_lines() {
        return None;
    }
    let line_byte = rope.line_to_byte(line);
    let line_slice = rope.line(line);
    let mut u16_seen: u32 = 0;
    let mut byte_off: usize = 0;
    for ch in line_slice.chars() {
        if u16_seen >= pos.character {
            break;
        }
        u16_seen += ch.len_utf16() as u32;
        byte_off += ch.len_utf8();
    }
    Some(line_byte + byte_off)
}

/// Apply a `WorkspaceEdit` to any currently-open buffers. Silently skips
/// edits targeting files that aren't open. Returns the number of files
/// successfully modified.
pub fn apply_workspace_edit(app: &mut App, edit: WorkspaceEdit) -> usize {
    // Collect per-URI edit lists from either `changes` or `document_changes`.
    let mut per_file: Vec<(Uri, Vec<TextEdit>)> = Vec::new();
    if let Some(changes) = edit.changes {
        for (uri, edits) in changes {
            per_file.push((uri, edits));
        }
    }
    if let Some(doc_changes) = edit.document_changes {
        match doc_changes {
            DocumentChanges::Edits(edits) => {
                for te in edits {
                    let uri = te.text_document.uri;
                    let plain: Vec<TextEdit> = te
                        .edits
                        .into_iter()
                        .map(|e| match e {
                            OneOf::Left(t) => t,
                            OneOf::Right(a) => a.text_edit,
                        })
                        .collect();
                    per_file.push((uri, plain));
                }
            }
            DocumentChanges::Operations(_) => {
                // create/rename/delete file ops — not supported here.
            }
        }
    }

    let mut modified = 0usize;
    for (uri, mut edits) in per_file {
        let Some(buf_idx) = find_buffer_by_uri(app, &uri) else {
            continue;
        };
        // Resolve every edit to byte ranges *before* mutating the rope.
        let buf = &app.buffers[buf_idx];
        let mut resolved: Vec<(usize, usize, String)> = Vec::new();
        let mut ok = true;
        for e in edits.drain(..) {
            let Some(s) = lsp_pos_to_byte(&buf.rope, e.range.start) else {
                ok = false;
                break;
            };
            let Some(en) = lsp_pos_to_byte(&buf.rope, e.range.end) else {
                ok = false;
                break;
            };
            resolved.push((s, en, e.new_text));
        }
        if !ok {
            continue;
        }
        // Apply in reverse start-byte order so earlier offsets remain valid.
        resolved.sort_by(|a, b| b.0.cmp(&a.0));
        let buf = &mut app.buffers[buf_idx];
        for (s, en, text) in resolved {
            if en > s {
                buf.delete(s..en);
            }
            if !text.is_empty() {
                buf.insert(s, &text);
            }
        }
        modified += 1;
    }
    modified
}

fn find_buffer_by_uri(app: &App, uri: &Uri) -> Option<usize> {
    // Fast path: already-opened buffer with matching lsp_uri.
    for (i, b) in app.buffers.iter().enumerate() {
        if b.lsp_uri.as_ref().map(|u| u.as_str()) == Some(uri.as_str()) {
            return Some(i);
        }
    }
    // Fallback: compare canonical paths.
    let url = url::Url::parse(uri.as_str()).ok()?;
    let target = url.to_file_path().ok()?;
    let target_canon = std::fs::canonicalize(&target).unwrap_or(target);
    app.buffers.iter().position(|b| {
        b.path
            .as_deref()
            .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()))
            == Some(target_canon.clone())
    })
}
