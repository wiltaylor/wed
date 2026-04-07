//! Key dispatch — converts a [`Key`] event into editor state mutations
//! based on the current [`EditorMode`] and pending state.
//!
//! Operates directly on `App` so it can mutate buffer + view + mode +
//! pending in one place; commands registered in `register_editor_commands`
//! are thin wrappers that call into the same primitives via [`Editor`].

use crate::app::App;
use crate::commands::context::LastChange;
use crate::editor::buffer::{Buffer, Point};
use crate::editor::motions;
use crate::editor::ops;
use crate::editor::registers::{RegisterEntry, YankKind};
use crate::editor::search::{search_next, search_prev};
use crate::editor::text_objects;
use crate::editor::Cursor;
use crate::input::keys::Key;
use crate::input::mode::{EditorMode, Operator, PendingKey, VisualKind};

/// View-agnostic helper: returns the active buffer index and a (row,col) cursor.
fn active_buffer_index(app: &App) -> Option<usize> {
    if app.buffers.is_empty() {
        None
    } else {
        Some(0)
    }
}

fn cursor_of(app: &App) -> Cursor {
    // Try to read from active view; fall back to (0,0).
    if let Some(tab) = app.layout.active_tab() {
        if let Some(view) = tab.root.find(tab.active_view) {
            return Cursor {
                row: view.cursor.0,
                col: view.cursor.1,
                want_col: app.want_col,
            };
        }
    }
    Cursor {
        row: 0,
        col: 0,
        want_col: app.want_col,
    }
}

fn set_cursor(app: &mut App, c: Cursor) {
    app.want_col = c.want_col;
    if let Some(tab) = app.layout.active_tab_mut() {
        let id = tab.active_view;
        if let Some(view) = tab.root.find_mut(id) {
            view.cursor = (c.row, c.col);
        }
    }
}

fn buf_mut(app: &mut App) -> Option<&mut Buffer> {
    let i = active_buffer_index(app)?;
    app.buffers.get_mut(i)
}

fn buf(app: &App) -> Option<&Buffer> {
    let i = active_buffer_index(app)?;
    app.buffers.get(i)
}

/// Top-level entry point.
pub struct KeyHandler;

impl KeyHandler {
    pub fn handle(app: &mut App, key: Key) {
        let mode = app.mode;
        match mode {
            EditorMode::Normal => Self::handle_normal(app, key),
            EditorMode::Insert => Self::handle_insert(app, key),
            EditorMode::Visual(kind) => Self::handle_visual(app, kind, key),
            EditorMode::Replace => Self::handle_replace(app, key),
            EditorMode::Pending(p) => Self::handle_pending(app, p, key),
            EditorMode::Operator(op) => Self::handle_operator_motion(app, op, key),
            EditorMode::Command | EditorMode::Search => {
                if key == Key::Esc {
                    app.mode = EditorMode::Normal;
                }
            }
        }
    }

    fn handle_normal(app: &mut App, key: Key) {
        // Count prefix
        if let Key::Char(c) = key {
            if c.is_ascii_digit() && (c != '0' || app.pending.count.is_some()) {
                app.pending.push_digit(c.to_digit(10).unwrap());
                return;
            }
        }
        let count = app.pending.count.unwrap_or(1).max(1);
        match key {
            Key::Char('h') | Key::Left => Self::motion(app, |b, c| motions::left(b, c, count)),
            Key::Char('l') | Key::Right => Self::motion(app, |b, c| motions::right(b, c, count)),
            Key::Char('j') | Key::Down => Self::motion(app, |b, c| motions::down(b, c, count)),
            Key::Char('k') | Key::Up => Self::motion(app, |b, c| motions::up(b, c, count)),
            Key::Char('w') => Self::motion(app, |b, c| motions::word_forward(b, c, count)),
            Key::Char('W') => Self::motion(app, |b, c| motions::word_forward_big(b, c, count)),
            Key::Char('b') => Self::motion(app, |b, c| motions::word_backward(b, c, count)),
            Key::Char('B') => Self::motion(app, |b, c| motions::word_backward_big(b, c, count)),
            Key::Char('e') => Self::motion(app, |b, c| motions::word_end(b, c, count)),
            Key::Char('E') => Self::motion(app, |b, c| motions::word_end_big(b, c, count)),
            Key::Char('0') => Self::motion(app, motions::line_start),
            Key::Char('^') => Self::motion(app, motions::line_first_non_blank),
            Key::Char('$') => Self::motion(app, motions::line_end),
            Key::Char('G') => {
                if let Some(n) = app.pending.count.take() {
                    Self::motion(app, |b, _| motions::goto_line(b, Cursor::default(), n));
                } else {
                    Self::motion(app, motions::buffer_bottom);
                }
            }
            Key::Char('g') => app.mode = EditorMode::Pending(PendingKey::G),
            Key::Char('f') => {
                app.mode = EditorMode::Pending(PendingKey::FindChar {
                    forward: true,
                    till: false,
                })
            }
            Key::Char('F') => {
                app.mode = EditorMode::Pending(PendingKey::FindChar {
                    forward: false,
                    till: false,
                })
            }
            Key::Char('t') => {
                app.mode = EditorMode::Pending(PendingKey::FindChar {
                    forward: true,
                    till: true,
                })
            }
            Key::Char('T') => {
                app.mode = EditorMode::Pending(PendingKey::FindChar {
                    forward: false,
                    till: true,
                })
            }
            Key::Char('%') => Self::motion(app, motions::match_bracket),
            Key::Char('(') => Self::motion(app, |b, c| motions::sentence_backward(b, c, count)),
            Key::Char(')') => Self::motion(app, |b, c| motions::sentence_forward(b, c, count)),
            Key::Char('{') => Self::motion(app, |b, c| motions::paragraph_backward(b, c, count)),
            Key::Char('}') => Self::motion(app, |b, c| motions::paragraph_forward(b, c, count)),
            Key::Char('H') => Self::motion(app, |b, _| Cursor::new(0, 0)),
            Key::Char('M') => Self::motion(app, |b, _| Cursor::new(b.line_count() / 2, 0)),
            Key::Char('L') => {
                Self::motion(app, |b, _| Cursor::new(b.line_count().saturating_sub(1), 0))
            }
            Key::Ctrl('d') => Self::motion(app, |b, c| motions::down(b, c, 10)),
            Key::Ctrl('u') => Self::motion(app, |b, c| motions::up(b, c, 10)),
            Key::Ctrl('f') => Self::motion(app, |b, c| motions::down(b, c, 20)),
            Key::Ctrl('b') => Self::motion(app, |b, c| motions::up(b, c, 20)),

            // Operators
            Key::Char('d') => {
                app.pending.operator = Some(Operator::Delete);
                app.mode = EditorMode::Operator(Operator::Delete);
            }
            Key::Char('c') => {
                app.pending.operator = Some(Operator::Change);
                app.mode = EditorMode::Operator(Operator::Change);
            }
            Key::Char('y') => {
                app.pending.operator = Some(Operator::Yank);
                app.mode = EditorMode::Operator(Operator::Yank);
            }
            Key::Char('>') => {
                app.pending.operator = Some(Operator::Indent);
                app.mode = EditorMode::Operator(Operator::Indent);
            }
            Key::Char('<') => {
                app.pending.operator = Some(Operator::Dedent);
                app.mode = EditorMode::Operator(Operator::Dedent);
            }

            // Line ops
            Key::Char('x') => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    let p = b.point_to_byte(Point {
                        row: cur.row,
                        col: cur.col,
                    });
                    let end = (p + 1).min(b.len_bytes());
                    if end > p {
                        b.delete(p..end);
                    }
                }
                app.last_change = LastChange {
                    kind: "x".into(),
                    count,
                    inserted: String::new(),
                };
                app.pending.reset();
            }
            Key::Char('p') => {
                let cur = cursor_of(app);
                let entry = buf(app).and_then(|b| b.registers.get('"').cloned());
                if let (Some(b), Some(e)) = (buf_mut(app), entry) {
                    let p = b.point_to_byte(Point {
                        row: cur.row,
                        col: cur.col,
                    });
                    let new = ops::paste_after(b, p, &e);
                    let np = b.byte_to_point(new);
                    set_cursor(app, Cursor::new(np.row, np.col));
                }
                app.last_change = LastChange {
                    kind: "p".into(),
                    count,
                    inserted: String::new(),
                };
                app.pending.reset();
            }
            Key::Char('P') => {
                let cur = cursor_of(app);
                let entry = buf(app).and_then(|b| b.registers.get('"').cloned());
                if let (Some(b), Some(e)) = (buf_mut(app), entry) {
                    let p = b.point_to_byte(Point {
                        row: cur.row,
                        col: cur.col,
                    });
                    let new = ops::paste_before(b, p, &e);
                    let np = b.byte_to_point(new);
                    set_cursor(app, Cursor::new(np.row, np.col));
                }
                app.pending.reset();
            }
            Key::Char('u') => {
                if let Some(b) = buf_mut(app) {
                    if let Some(byte) = b.undo() {
                        let p = b.byte_to_point(byte);
                        set_cursor(app, Cursor::new(p.row, p.col));
                    }
                }
                app.pending.reset();
            }
            Key::Ctrl('r') => {
                if let Some(b) = buf_mut(app) {
                    if let Some(byte) = b.redo() {
                        let p = b.byte_to_point(byte);
                        set_cursor(app, Cursor::new(p.row, p.col));
                    }
                }
                app.pending.reset();
            }

            // Mode transitions
            Key::Char('i') => Self::enter_insert(app),
            Key::Char('I') => {
                Self::motion(app, motions::line_first_non_blank);
                Self::enter_insert(app);
            }
            Key::Char('a') => {
                Self::motion(app, |b, c| motions::right(b, c, 1));
                Self::enter_insert(app);
            }
            Key::Char('A') => {
                Self::motion(app, |b, c| {
                    let max = b.line_len_chars(c.row);
                    Cursor::new(c.row, max)
                });
                Self::enter_insert(app);
            }
            Key::Char('o') => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    let line_end = b.line_len_chars(cur.row);
                    let pos = b.point_to_byte(Point {
                        row: cur.row,
                        col: line_end,
                    });
                    b.insert(pos, "\n");
                    set_cursor(app, Cursor::new(cur.row + 1, 0));
                }
                Self::enter_insert(app);
            }
            Key::Char('O') => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    let pos = b.point_to_byte(Point {
                        row: cur.row,
                        col: 0,
                    });
                    b.insert(pos, "\n");
                    set_cursor(app, Cursor::new(cur.row, 0));
                }
                Self::enter_insert(app);
            }
            Key::Char('v') => {
                app.pending.visual_anchor = Some(cursor_of(app));
                app.mode = EditorMode::Visual(VisualKind::Char);
            }
            Key::Char('V') => {
                app.pending.visual_anchor = Some(cursor_of(app));
                app.mode = EditorMode::Visual(VisualKind::Line);
            }
            Key::Ctrl('v') => {
                app.pending.visual_anchor = Some(cursor_of(app));
                app.mode = EditorMode::Visual(VisualKind::Block);
            }
            Key::Char('m') => app.mode = EditorMode::Pending(PendingKey::SetMark),
            Key::Char('\'') => app.mode = EditorMode::Pending(PendingKey::JumpMark),
            Key::Char(';') => {
                if let Some((ch, fwd, till)) = app.pending.last_find {
                    Self::motion(app, |b, c| motions::find_char(b, c, ch, fwd, till, count));
                }
            }
            Key::Char(',') => {
                if let Some((ch, fwd, till)) = app.pending.last_find {
                    Self::motion(app, |b, c| motions::find_char(b, c, ch, !fwd, till, count));
                }
            }
            Key::Char('R') => app.mode = EditorMode::Replace,
            Key::Char(':') => app.mode = EditorMode::Command,
            Key::Char('/') => app.mode = EditorMode::Search,
            Key::Char('n') => {
                let cur = cursor_of(app);
                if let Some(b) = buf(app) {
                    if let Some(c) = search_next(b, &app.search, cur) {
                        set_cursor(app, c);
                    }
                }
            }
            Key::Char('N') => {
                let cur = cursor_of(app);
                if let Some(b) = buf(app) {
                    if let Some(c) = search_prev(b, &app.search, cur) {
                        set_cursor(app, c);
                    }
                }
            }
            Key::Char('.') => Self::dot_repeat(app),
            Key::Esc => app.pending.reset(),
            _ => {}
        }
    }

    fn motion(app: &mut App, f: impl Fn(&Buffer, Cursor) -> Cursor) {
        let cur = cursor_of(app);
        if let Some(b) = buf(app) {
            let new = f(b, cur);
            set_cursor(app, new);
        }
        app.pending.count = None;
    }

    fn enter_insert(app: &mut App) {
        app.mode = EditorMode::Insert;
        if let Some(b) = buf_mut(app) {
            b.history.begin_batch();
        }
    }

    fn exit_insert(app: &mut App) {
        if let Some(b) = buf_mut(app) {
            b.history.commit_batch();
        }
        app.mode = EditorMode::Normal;
    }

    fn handle_insert(app: &mut App, key: Key) {
        match key {
            Key::Esc => Self::exit_insert(app),
            Key::Char(c) => Self::insert_str(app, &c.to_string()),
            Key::Enter => Self::insert_str(app, "\n"),
            Key::Tab => {
                let tab = "    ";
                Self::insert_str(app, tab);
            }
            Key::Backspace => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    let pos = b.point_to_byte(Point {
                        row: cur.row,
                        col: cur.col,
                    });
                    if pos > 0 {
                        b.delete(pos - 1..pos);
                        let np = b.byte_to_point(pos - 1);
                        set_cursor(app, Cursor::new(np.row, np.col));
                    }
                }
            }
            Key::Ctrl('w') => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    let bw = motions::word_backward(b, cur, 1);
                    let from = b.point_to_byte(Point {
                        row: bw.row,
                        col: bw.col,
                    });
                    let to = b.point_to_byte(Point {
                        row: cur.row,
                        col: cur.col,
                    });
                    if to > from {
                        b.delete(from..to);
                    }
                    let np = b.byte_to_point(from);
                    set_cursor(app, Cursor::new(np.row, np.col));
                }
            }
            Key::Ctrl('u') => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    let from = b.point_to_byte(Point {
                        row: cur.row,
                        col: 0,
                    });
                    let to = b.point_to_byte(Point {
                        row: cur.row,
                        col: cur.col,
                    });
                    if to > from {
                        b.delete(from..to);
                    }
                    set_cursor(app, Cursor::new(cur.row, 0));
                }
            }
            _ => {}
        }
    }

    fn insert_str(app: &mut App, s: &str) {
        let cur = cursor_of(app);
        if let Some(b) = buf_mut(app) {
            let pos = b.point_to_byte(Point {
                row: cur.row,
                col: cur.col,
            });
            b.insert(pos, s);
            let np = b.byte_to_point(pos + s.len());
            set_cursor(app, Cursor::new(np.row, np.col));
        }
        app.last_change.inserted.push_str(s);
        app.last_change.kind = "insert".into();
    }

    fn handle_replace(app: &mut App, key: Key) {
        match key {
            Key::Esc => app.mode = EditorMode::Normal,
            Key::Char(c) => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    let pos = b.point_to_byte(Point {
                        row: cur.row,
                        col: cur.col,
                    });
                    let end = (pos + 1).min(b.len_bytes());
                    if end > pos {
                        b.delete(pos..end);
                    }
                    b.insert(pos, &c.to_string());
                    let np = b.byte_to_point(pos + c.len_utf8());
                    set_cursor(app, Cursor::new(np.row, np.col));
                }
            }
            _ => {}
        }
    }

    fn handle_pending(app: &mut App, p: PendingKey, key: Key) {
        match (p, key) {
            (PendingKey::G, Key::Char('g')) => {
                Self::motion(app, |_, _| Cursor::new(0, 0));
                app.mode = EditorMode::Normal;
            }
            (PendingKey::G, Key::Char('c')) => {
                // gc: comment toggle on motion (operator)
                app.pending.operator = Some(Operator::Comment);
                app.mode = EditorMode::Operator(Operator::Comment);
            }
            (PendingKey::FindChar { forward, till }, Key::Char(ch)) => {
                let n = app.pending.take_count();
                Self::motion(app, |b, c| motions::find_char(b, c, ch, forward, till, n));
                app.pending.last_find = Some((ch, forward, till));
                app.mode = EditorMode::Normal;
            }
            (_, Key::Esc) => {
                app.mode = EditorMode::Normal;
                app.pending.reset();
            }
            _ => {
                app.mode = EditorMode::Normal;
            }
        }
    }

    fn handle_operator_motion(app: &mut App, op: Operator, key: Key) {
        // Handle digit count.
        if let Key::Char(c) = key {
            if c.is_ascii_digit() && (c != '0' || app.pending.count.is_some()) {
                app.pending.push_digit(c.to_digit(10).unwrap());
                return;
            }
        }
        let count = app.pending.count.unwrap_or(1).max(1);

        // Doubled-letter line operator: dd, cc, yy, >>, <<
        let line_op = matches!(
            (op, key),
            (Operator::Delete, Key::Char('d'))
                | (Operator::Change, Key::Char('c'))
                | (Operator::Yank, Key::Char('y'))
                | (Operator::Indent, Key::Char('>'))
                | (Operator::Dedent, Key::Char('<'))
                | (Operator::Comment, Key::Char('c'))
        );
        if line_op {
            let cur = cursor_of(app);
            let start_row = cur.row;
            let end_row = (cur.row + count - 1).min(
                buf(app)
                    .map(|b| b.line_count().saturating_sub(1))
                    .unwrap_or(0),
            );
            Self::apply_line_op(app, op, start_row, end_row);
            app.last_change = LastChange {
                kind: format!("{:?}_line", op),
                count,
                inserted: String::new(),
            };
            app.pending.reset();
            app.mode = if matches!(op, Operator::Change) {
                EditorMode::Insert
            } else {
                EditorMode::Normal
            };
            return;
        }

        // Text object: i/a + <delim>
        if let Key::Char(first) = key {
            if first == 'i' || first == 'a' {
                app.pending.buf.push(first);
                return;
            }
        }
        if !app.pending.buf.is_empty() {
            // expect a delimiter / object char
            let prefix = app.pending.buf.chars().next().unwrap();
            let inner = prefix == 'i';
            let cur = cursor_of(app);
            let range_opt = if let Key::Char(ch) = key {
                let buf_ref = buf(app).unwrap();
                match ch {
                    'w' => {
                        if inner {
                            text_objects::inner_word(buf_ref, cur)
                        } else {
                            text_objects::around_word(buf_ref, cur)
                        }
                    }
                    '"' => {
                        if inner {
                            text_objects::inner_pair(buf_ref, cur, '"', '"')
                        } else {
                            text_objects::around_pair(buf_ref, cur, '"', '"')
                        }
                    }
                    '\'' => {
                        if inner {
                            text_objects::inner_pair(buf_ref, cur, '\'', '\'')
                        } else {
                            text_objects::around_pair(buf_ref, cur, '\'', '\'')
                        }
                    }
                    '(' | ')' => {
                        if inner {
                            text_objects::inner_pair(buf_ref, cur, '(', ')')
                        } else {
                            text_objects::around_pair(buf_ref, cur, '(', ')')
                        }
                    }
                    '[' | ']' => {
                        if inner {
                            text_objects::inner_pair(buf_ref, cur, '[', ']')
                        } else {
                            text_objects::around_pair(buf_ref, cur, '[', ']')
                        }
                    }
                    '{' | '}' => {
                        if inner {
                            text_objects::inner_pair(buf_ref, cur, '{', '}')
                        } else {
                            text_objects::around_pair(buf_ref, cur, '{', '}')
                        }
                    }
                    '<' | '>' => {
                        if inner {
                            text_objects::inner_pair(buf_ref, cur, '<', '>')
                        } else {
                            text_objects::around_pair(buf_ref, cur, '<', '>')
                        }
                    }
                    'p' => {
                        if inner {
                            text_objects::inner_paragraph(buf_ref, cur)
                        } else {
                            text_objects::around_paragraph(buf_ref, cur)
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };
            if let Some(r) = range_opt {
                Self::apply_byte_range_op(app, op, r.start, r.end);
            }
            app.pending.reset();
            app.mode = if matches!(op, Operator::Change) {
                EditorMode::Insert
            } else {
                EditorMode::Normal
            };
            return;
        }

        // Otherwise treat key as a motion → compute end cursor → byte range from cur..end
        let start = cursor_of(app);
        let end = match key {
            Key::Char('w') => motions::word_forward(buf(app).unwrap(), start, count),
            Key::Char('b') => motions::word_backward(buf(app).unwrap(), start, count),
            Key::Char('e') => {
                let mut e = motions::word_end(buf(app).unwrap(), start, count);
                e.col += 1;
                e
            }
            Key::Char('h') => motions::left(buf(app).unwrap(), start, count),
            Key::Char('l') => motions::right(buf(app).unwrap(), start, count),
            Key::Char('$') => motions::line_end(buf(app).unwrap(), start),
            Key::Char('0') => motions::line_start(buf(app).unwrap(), start),
            Key::Esc => {
                app.pending.reset();
                app.mode = EditorMode::Normal;
                return;
            }
            _ => {
                app.pending.reset();
                app.mode = EditorMode::Normal;
                return;
            }
        };
        let bs = buf(app).unwrap().point_to_byte(Point {
            row: start.row,
            col: start.col,
        });
        let be = buf(app).unwrap().point_to_byte(Point {
            row: end.row,
            col: end.col,
        });
        let (lo, hi) = if bs <= be { (bs, be) } else { (be, bs) };
        Self::apply_byte_range_op(app, op, lo, hi);
        app.last_change = LastChange {
            kind: format!("{:?}_motion", op),
            count,
            inserted: String::new(),
        };
        app.pending.reset();
        app.mode = if matches!(op, Operator::Change) {
            EditorMode::Insert
        } else {
            EditorMode::Normal
        };
    }

    fn apply_byte_range_op(app: &mut App, op: Operator, lo: usize, hi: usize) {
        match op {
            Operator::Delete | Operator::Change => {
                if let Some(b) = buf_mut(app) {
                    let text = b.slice_bytes(lo..hi);
                    b.registers.set_unnamed(text, YankKind::Char);
                    b.delete(lo..hi);
                    let p = b.byte_to_point(lo);
                    set_cursor(app, Cursor::new(p.row, p.col));
                }
            }
            Operator::Yank => {
                if let Some(b) = buf_mut(app) {
                    let text = b.slice_bytes(lo..hi);
                    b.registers.set_unnamed(text, YankKind::Char);
                }
            }
            Operator::Indent | Operator::Dedent | Operator::Comment => {
                let (sp, ep) = (
                    buf(app).unwrap().byte_to_point(lo),
                    buf(app).unwrap().byte_to_point(hi),
                );
                Self::apply_line_op(app, op, sp.row, ep.row);
            }
        }
    }

    fn apply_line_op(app: &mut App, op: Operator, start_row: usize, end_row: usize) {
        match op {
            Operator::Delete | Operator::Change => {
                if let Some(b) = buf_mut(app) {
                    let from = b.point_to_byte(Point {
                        row: start_row,
                        col: 0,
                    });
                    let next_row = end_row + 1;
                    let to = if next_row < b.line_count() {
                        b.point_to_byte(Point {
                            row: next_row,
                            col: 0,
                        })
                    } else {
                        b.len_bytes()
                    };
                    let text = b.slice_bytes(from..to);
                    b.registers.set_unnamed(text, YankKind::Line);
                    b.delete(from..to);
                    let p = b.byte_to_point(from);
                    set_cursor(app, Cursor::new(p.row, p.col));
                }
            }
            Operator::Yank => {
                if let Some(b) = buf_mut(app) {
                    let from = b.point_to_byte(Point {
                        row: start_row,
                        col: 0,
                    });
                    let next_row = end_row + 1;
                    let to = if next_row < b.line_count() {
                        b.point_to_byte(Point {
                            row: next_row,
                            col: 0,
                        })
                    } else {
                        b.len_bytes()
                    };
                    let text = b.slice_bytes(from..to);
                    b.registers.set_unnamed(text, YankKind::Line);
                }
            }
            Operator::Indent => {
                if let Some(b) = buf_mut(app) {
                    ops::indent_rows(b, start_row..=end_row, "    ");
                }
            }
            Operator::Dedent => {
                if let Some(b) = buf_mut(app) {
                    ops::dedent_rows(b, start_row..=end_row, 4);
                }
            }
            Operator::Comment => {
                let lang = buf(app).and_then(|b| b.language_id.clone());
                let cs = ops::comment_string_for(lang.as_deref());
                if let Some(b) = buf_mut(app) {
                    ops::comment_toggle_rows(b, start_row..=end_row, cs);
                }
            }
        }
    }

    fn handle_visual(app: &mut App, kind: VisualKind, key: Key) {
        match key {
            Key::Esc => app.mode = EditorMode::Normal,
            Key::Char('h') => Self::motion(app, |b, c| motions::left(b, c, 1)),
            Key::Char('l') => Self::motion(app, |b, c| motions::right(b, c, 1)),
            Key::Char('j') => Self::motion(app, |b, c| motions::down(b, c, 1)),
            Key::Char('k') => Self::motion(app, |b, c| motions::up(b, c, 1)),
            // d/c/y on visual: TODO use anchor; without an anchor stored, treat as line for V
            Key::Char('d') | Key::Char('c') | Key::Char('y') => {
                app.mode = EditorMode::Normal;
            }
            _ => {}
        }
    }

    fn dot_repeat(app: &mut App) {
        let lc = app.last_change.clone();
        if lc.kind == "insert" && !lc.inserted.is_empty() {
            Self::insert_str(app, &lc.inserted);
        }
        // Other repeat kinds are placeholder TODOs.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    fn app_with(text: &str) -> App {
        let mut app = App::new();
        app.buffers.push(Buffer::from_str(text));
        // Build a minimal layout view so cursor_of/set_cursor work.
        use crate::app::ViewId;
        use crate::layout::{SplitNode, Tab, View};
        let view = View::new(ViewId(1), crate::app::BufferId(0));
        let root = SplitNode::Leaf(view);
        app.layout.tabs.push(Tab::new("t", root, ViewId(1)));
        app
    }

    #[test]
    fn h_l_move_cursor() {
        let mut app = app_with("hello");
        KeyHandler::handle(&mut app, Key::Char('l'));
        KeyHandler::handle(&mut app, Key::Char('l'));
        assert_eq!(cursor_of(&app).col, 2);
        KeyHandler::handle(&mut app, Key::Char('h'));
        assert_eq!(cursor_of(&app).col, 1);
    }

    #[test]
    fn count_prefix_3l() {
        let mut app = app_with("abcdef");
        KeyHandler::handle(&mut app, Key::Char('3'));
        KeyHandler::handle(&mut app, Key::Char('l'));
        assert_eq!(cursor_of(&app).col, 3);
    }

    #[test]
    fn dw_deletes_word() {
        let mut app = app_with("foo bar");
        KeyHandler::handle(&mut app, Key::Char('d'));
        KeyHandler::handle(&mut app, Key::Char('w'));
        assert_eq!(app.buffers[0].rope.to_string(), "bar");
    }

    #[test]
    fn d3w_deletes_three_words() {
        let mut app = app_with("a b c d e");
        KeyHandler::handle(&mut app, Key::Char('d'));
        KeyHandler::handle(&mut app, Key::Char('3'));
        KeyHandler::handle(&mut app, Key::Char('w'));
        // Expect first three "words" gone
        assert!(app.buffers[0].rope.to_string().starts_with("d"));
    }

    #[test]
    fn ci_quote_text_object() {
        let mut app = app_with("foo \"bar\" baz");
        // move to inside the quote
        for _ in 0..6 {
            KeyHandler::handle(&mut app, Key::Char('l'));
        }
        KeyHandler::handle(&mut app, Key::Char('c'));
        KeyHandler::handle(&mut app, Key::Char('i'));
        KeyHandler::handle(&mut app, Key::Char('"'));
        assert_eq!(app.buffers[0].rope.to_string(), "foo \"\" baz");
        assert_eq!(app.mode, EditorMode::Insert);
    }

    #[test]
    fn da_paren_text_object() {
        let mut app = app_with("call(a, b) end");
        for _ in 0..6 {
            KeyHandler::handle(&mut app, Key::Char('l'));
        }
        KeyHandler::handle(&mut app, Key::Char('d'));
        KeyHandler::handle(&mut app, Key::Char('a'));
        KeyHandler::handle(&mut app, Key::Char('('));
        assert_eq!(app.buffers[0].rope.to_string(), "call end");
    }

    #[test]
    fn dd_deletes_line() {
        let mut app = app_with("a\nb\nc\n");
        KeyHandler::handle(&mut app, Key::Char('j'));
        KeyHandler::handle(&mut app, Key::Char('d'));
        KeyHandler::handle(&mut app, Key::Char('d'));
        assert_eq!(app.buffers[0].rope.to_string(), "a\nc\n");
    }

    #[test]
    fn yank_paste() {
        let mut app = app_with("hello\nworld\n");
        KeyHandler::handle(&mut app, Key::Char('y'));
        KeyHandler::handle(&mut app, Key::Char('y'));
        KeyHandler::handle(&mut app, Key::Char('p'));
        // yy on line 0 yanks "hello\n"; p inserts on the next line.
        assert_eq!(app.buffers[0].rope.to_string(), "hello\nhello\nworld\n");
        // Unnamed register holds the yanked line.
        assert_eq!(app.buffers[0].registers.get('"').unwrap().text, "hello\n");
    }

    #[test]
    fn insert_then_esc_undo() {
        let mut app = app_with("");
        KeyHandler::handle(&mut app, Key::Char('i'));
        KeyHandler::handle(&mut app, Key::Char('h'));
        KeyHandler::handle(&mut app, Key::Char('i'));
        KeyHandler::handle(&mut app, Key::Esc);
        assert_eq!(app.buffers[0].rope.to_string(), "hi");
        KeyHandler::handle(&mut app, Key::Char('u'));
        assert_eq!(app.buffers[0].rope.to_string(), "");
    }

    #[test]
    fn dot_repeat_insert() {
        let mut app = app_with("");
        KeyHandler::handle(&mut app, Key::Char('i'));
        KeyHandler::handle(&mut app, Key::Char('a'));
        KeyHandler::handle(&mut app, Key::Esc);
        KeyHandler::handle(&mut app, Key::Char('.'));
        assert!(app.buffers[0].rope.to_string().contains("aa"));
    }

    #[test]
    fn search_n_jumps() {
        let mut app = app_with("foo bar foo");
        app.search.set("foo", true);
        KeyHandler::handle(&mut app, Key::Char('n'));
        assert_eq!(cursor_of(&app).col, 8);
    }
}
