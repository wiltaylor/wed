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

fn op_char(op: Operator) -> char {
    match op {
        Operator::Delete => 'd',
        Operator::Change => 'c',
        Operator::Yank => 'y',
        Operator::Indent => '>',
        Operator::Dedent => '<',
        Operator::Comment => 'g',
    }
}

fn motion_chars(k: &Key) -> Vec<char> {
    if let Key::Char(c) = k {
        vec![*c]
    } else {
        Vec::new()
    }
}

/// Top-level entry point.
pub struct KeyHandler;

impl KeyHandler {
    pub fn handle(app: &mut App, key: Key) {
        app.status_message = None;
        // Picker overlay swallows all input.
        if app.picker.is_some() {
            Self::handle_picker(app, key);
            return;
        }
        // Sidebar focus swallows all input until released.
        if app.sidebar_focused {
            Self::handle_sidebar(app, key);
            return;
        }
        // Active leader sequence: route through the leader trie.
        if app.leader_seq.is_some() {
            Self::handle_leader(app, key);
            return;
        }
        // Leader key in normal mode opens a fresh leader sequence.
        if matches!(app.mode, EditorMode::Normal) && key == app.keybindings.leader_key {
            app.leader_seq = Some(Vec::new());
            return;
        }
        let mode = app.mode;
        match mode {
            EditorMode::Normal => Self::handle_normal(app, key),
            EditorMode::Insert => Self::handle_insert(app, key),
            EditorMode::Visual(kind) => Self::handle_visual(app, kind, key),
            EditorMode::Replace => Self::handle_replace(app, key),
            EditorMode::Pending(p) => Self::handle_pending(app, p, key),
            EditorMode::Operator(op) => Self::handle_operator_motion(app, op, key),
            EditorMode::Command | EditorMode::Search => Self::handle_command_line(app, key),
        }
    }

    pub fn mouse(app: &mut App, ev: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }
        let (col, row) = (ev.column, ev.row);
        let in_rect = |r: ratatui::layout::Rect| {
            r.width > 0
                && r.height > 0
                && col >= r.x
                && col < r.x + r.width
                && row >= r.y
                && row < r.y + r.height
        };

        // Tabline close-button click: close that tab.
        for (i, r) in app.last_tab_close_rects.clone().iter().enumerate() {
            if in_rect(*r) {
                if app.layout.tabs.len() > 1 {
                    app.layout.tabs.remove(i);
                    if app.layout.active_tab >= app.layout.tabs.len() {
                        app.layout.active_tab = app.layout.tabs.len() - 1;
                    } else if app.layout.active_tab > i {
                        app.layout.active_tab -= 1;
                    }
                } else if let Some(b) = app.buffers.iter().find(|b| b.dirty) {
                    let name = b
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "[No Name]".to_string());
                    app.status_message = Some((
                        format!("unsaved changes in {name} (use :q! to force)"),
                        true,
                    ));
                } else {
                    app.should_quit = true;
                }
                return;
            }
        }
        // Tabline click: switch to the clicked tab.
        for (i, r) in app.last_tab_rects.clone().iter().enumerate() {
            if in_rect(*r) {
                app.layout.active_tab = i;
                return;
            }
        }

        // Sidebar click: select row, second click on the same row activates.
        if in_rect(app.last_left_sidebar_rect) {
            app.sidebar_focused = true;
            // Sidebar is rendered with a Block border (1-cell padding).
            let inner_y = app.last_left_sidebar_rect.y + 1;
            let clicked_row = row.saturating_sub(inner_y) as usize;
            let sb = &mut app.layout.left_sidebar;
            if let Some(pane) = sb.panes.get_mut(sb.active) {
                let len = pane.row_count();
                if len > 0 {
                    let target = clicked_row.min(len - 1);
                    // We don't have access to the previous selection through
                    // the trait; emulate "second click activates" by always
                    // selecting then activating when the click row matches.
                    pane.select_row(target);
                    if clicked_row < len {
                        // First click selects; the user can click again to
                        // activate. Track via a per-app last-clicked row.
                        if app.last_sidebar_click_row == Some(target) {
                            pane.activate_selected();
                            app.last_sidebar_click_row = None;
                        } else {
                            app.last_sidebar_click_row = Some(target);
                        }
                    }
                }
            }
            // After potential activation, check if a file was opened.
            let opened = {
                let sb = &mut app.layout.left_sidebar;
                sb.panes
                    .get_mut(sb.active)
                    .and_then(|p| p.take_opened_path())
            };
            if let Some(path) = opened {
                match app.open_file_in_new_tab(&path) {
                    Ok(()) => app.sidebar_focused = false,
                    Err(e) => {
                        app.status_message = Some((format!("open failed: {e}"), true))
                    }
                }
            }
            return;
        }

        // Editor click: figure out which view leaf was hit, set cursor.
        let mut hit: Option<(crate::app::ViewId, ratatui::layout::Rect)> = None;
        for (vid, r) in &app.last_editor_view_rects {
            if in_rect(*r) {
                hit = Some((*vid, *r));
                break;
            }
        }
        if let Some((vid, rect)) = hit {
            // Compute gutter width to translate screen col → buffer col.
            let buf_lines = if let Some(tab) = app.layout.active_tab() {
                tab.root
                    .find(vid)
                    .and_then(|v| app.buffers.get(v.buffer_id.0 as usize))
                    .map(|b| b.rope.len_lines())
                    .unwrap_or(0)
            } else {
                0
            };
            let gw = crate::render::editor_view::gutter_width(
                crate::render::editor_view::line_number_style(app),
                buf_lines.max(1),
            );
            if let Some(tab) = app.layout.active_tab_mut() {
                tab.active_view = vid;
                if let Some(view) = tab.root.find_mut(vid) {
                    let local_row = (row - rect.y) as usize;
                    let local_col = col.saturating_sub(rect.x);
                    let (br, bc) = view.screen_to_buffer(local_row as u16, local_col, gw);
                    view.cursor = (br, bc);
                }
            }
            app.sidebar_focused = false;
        }
    }

    fn handle_leader(app: &mut App, key: Key) {
        use crate::config::keybindings::Resolution;
        if key == Key::Esc {
            app.leader_seq = None;
            return;
        }
        let mut seq = app.leader_seq.take().unwrap_or_default();
        seq.push(key);
        let mut full = vec![app.keybindings.leader_key];
        full.extend(seq.iter().copied());
        match app.keybindings.resolve(EditorMode::Normal, &full) {
            Resolution::Pending => {
                app.leader_seq = Some(seq);
            }
            Resolution::Match(cmd) => {
                Self::run_leader_command(app, &cmd.command);
            }
            Resolution::NoMatch => {
                app.status_message =
                    Some((format!("no leader binding for {:?}", seq), true));
            }
        }
    }

    fn run_leader_command(app: &mut App, name: &str) {
        match name {
            "search.files" => {
                let root =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                app.picker = Some(crate::panes::picker::picker_files(&root));
                app.picker_query.clear();
            }
            "sidebar.left_toggle" => {
                let sb = &mut app.layout.left_sidebar;
                if sb.panes.is_empty() {
                    sb.panes
                        .push(Box::new(crate::panes::file_browser::FileBrowserPane::default()));
                }
                sb.open = !sb.open;
                app.sidebar_focused = sb.open;
            }
            other => {
                app.status_message =
                    Some((format!("leader command not wired: {other}"), true));
            }
        }
    }

    fn handle_picker(app: &mut App, key: Key) {
        let Some(picker) = app.picker.as_mut() else {
            return;
        };
        match key {
            Key::Esc => {
                app.picker = None;
                app.picker_query.clear();
            }
            Key::Up => picker.move_up(),
            Key::Down => picker.move_down(),
            Key::Backspace => {
                app.picker_query.pop();
                picker.set_query(app.picker_query.clone());
            }
            Key::Char(c) => {
                app.picker_query.push(c);
                picker.set_query(app.picker_query.clone());
            }
            Key::Enter => {
                let chosen = picker.current().cloned();
                app.picker = None;
                app.picker_query.clear();
                if let Some(path) = chosen {
                    if let Err(e) = app.open_file_in_new_tab(&path) {
                        app.status_message = Some((format!("open failed: {e}"), true));
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_sidebar(app: &mut App, key: Key) {
        // Esc returns focus to the editor without closing.
        if key == Key::Esc {
            app.sidebar_focused = false;
            return;
        }
        // Let the user drop into : / / from the sidebar — unfocus and
        // dispatch the key as if it had been pressed in normal mode.
        if matches!(key, Key::Char(':') | Key::Char('/') | Key::Char('?')) {
            app.sidebar_focused = false;
            Self::handle_normal(app, key);
            return;
        }
        {
            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
            let code = match key {
                Key::Char(c) => Some(KeyCode::Char(c)),
                Key::Enter => Some(KeyCode::Enter),
                Key::Up => Some(KeyCode::Up),
                Key::Down => Some(KeyCode::Down),
                Key::Left => Some(KeyCode::Left),
                Key::Right => Some(KeyCode::Right),
                Key::Backspace => Some(KeyCode::Backspace),
                _ => None,
            };
            if let Some(code) = code {
                let sb = &mut app.layout.left_sidebar;
                if let Some(pane) = sb.panes.get_mut(sb.active) {
                    pane.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
                }
            }
        }
        // After dispatch, check if the active pane opened a file.
        let opened = {
            let sb = &mut app.layout.left_sidebar;
            sb.panes
                .get_mut(sb.active)
                .and_then(|p| p.take_opened_path())
        };
        if let Some(path) = opened {
            match app.open_file_in_new_tab(&path) {
                Ok(()) => app.sidebar_focused = false,
                Err(e) => {
                    app.status_message = Some((format!("open failed: {e}"), true));
                }
            }
        }
    }

    fn handle_command_line(app: &mut App, key: Key) {
        match key {
            Key::Esc => {
                app.command_line.clear();
                app.mode = EditorMode::Normal;
            }
            Key::Backspace => {
                if app.command_line.input.is_empty() {
                    app.mode = EditorMode::Normal;
                } else {
                    app.command_line.backspace();
                }
            }
            Key::Left => app.command_line.move_left(),
            Key::Right => app.command_line.move_right(),
            Key::Up => app.command_line.history_prev(),
            Key::Down => app.command_line.history_next(),
            Key::Tab => {
                if matches!(app.mode, EditorMode::Command) {
                    let reg = std::mem::take(&mut app.commands);
                    app.command_line.complete(&reg);
                    app.commands = reg;
                }
            }
            Key::Char(c) => app.command_line.insert_char(c),
            Key::Enter => {
                let was_search = matches!(app.mode, EditorMode::Search);
                if was_search {
                    let pat = std::mem::take(&mut app.command_line.input);
                    app.command_line.cursor = 0;
                    if !pat.is_empty() {
                        app.command_line.history.push(pat.clone());
                        app.search.set(&pat, true);
                        let cur = cursor_of(app);
                        if let Some(b) = buf(app) {
                            if let Some(c) = search_next(b, &app.search, cur) {
                                set_cursor(app, c);
                            }
                        }
                    }
                    app.mode = EditorMode::Normal;
                } else {
                    let registry = std::mem::take(&mut app.commands);
                    let App {
                        buffers,
                        layout,
                        mode,
                        config,
                        event_tx,
                        should_quit,
                        command_line,
                        ..
                    } = app;
                    let mut ctx = crate::commands::CommandContext::new(
                        buffers, layout, mode, config, event_tx, should_quit,
                    );
                    let result = command_line.accept(&registry, &mut ctx);
                    app.commands = registry;
                    match result {
                        Ok(()) => app.status_message = None,
                        Err(e) => app.status_message = Some((format!("{e}"), true)),
                    }
                    if matches!(app.mode, EditorMode::Command) {
                        app.mode = EditorMode::Normal;
                    }
                }
            }
            _ => {}
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
                    operator: None,
                    motion: Vec::new(),
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
                    operator: None,
                    motion: Vec::new(),
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
            Key::Char('r') => app.mode = EditorMode::Pending(PendingKey::Replace),
            Key::Char('R') => app.mode = EditorMode::Replace,
            Key::Char(':') => {
                app.command_line.clear();
                app.mode = EditorMode::Command;
            }
            Key::Char('/') => {
                app.command_line.clear();
                app.mode = EditorMode::Search;
            }
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
            (PendingKey::SetMark, Key::Char(ch)) if ch.is_ascii_lowercase() => {
                let cur = cursor_of(app);
                if let Some(b) = buf_mut(app) {
                    b.marks.set(ch, cur);
                }
                app.mode = EditorMode::Normal;
            }
            (PendingKey::JumpMark, Key::Char(ch)) if ch.is_ascii_lowercase() => {
                let target = buf(app).and_then(|b| b.marks.get(ch));
                if let Some(c) = target {
                    set_cursor(app, c);
                }
                app.mode = EditorMode::Normal;
            }
            (PendingKey::Replace, Key::Char(ch)) => {
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
                    b.insert(pos, &ch.to_string());
                }
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
                kind: format!("{op:?}_line"),
                count,
                inserted: String::new(),
                operator: Some(op_char(op)),
                motion: vec![op_char(op)],
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
            kind: format!("{op:?}_motion"),
            count,
            inserted: String::new(),
            operator: Some(op_char(op)),
            motion: motion_chars(&key),
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
            Key::Esc => {
                app.pending.visual_anchor = None;
                app.mode = EditorMode::Normal;
            }
            Key::Char('h') => Self::motion(app, |b, c| motions::left(b, c, 1)),
            Key::Char('l') => Self::motion(app, |b, c| motions::right(b, c, 1)),
            Key::Char('j') => Self::motion(app, |b, c| motions::down(b, c, 1)),
            Key::Char('k') => Self::motion(app, |b, c| motions::up(b, c, 1)),
            Key::Char('w') => Self::motion(app, |b, c| motions::word_forward(b, c, 1)),
            Key::Char('b') => Self::motion(app, |b, c| motions::word_backward(b, c, 1)),
            Key::Char('$') => Self::motion(app, motions::line_end),
            Key::Char('0') => Self::motion(app, motions::line_start),
            Key::Char('d') | Key::Char('c') | Key::Char('y') => {
                let op = match key {
                    Key::Char('d') => Operator::Delete,
                    Key::Char('c') => Operator::Change,
                    _ => Operator::Yank,
                };
                Self::apply_visual_op(app, kind, op);
                app.pending.visual_anchor = None;
                app.mode = if matches!(op, Operator::Change) {
                    EditorMode::Insert
                } else {
                    EditorMode::Normal
                };
            }
            _ => {}
        }
    }

    fn apply_visual_op(app: &mut App, kind: VisualKind, op: Operator) {
        let head = cursor_of(app);
        let anchor = app.pending.visual_anchor.unwrap_or(head);
        match kind {
            VisualKind::Line => {
                let (s, e) = if anchor.row <= head.row {
                    (anchor.row, head.row)
                } else {
                    (head.row, anchor.row)
                };
                Self::apply_line_op(app, op, s, e);
            }
            VisualKind::Char | VisualKind::Block => {
                let (a, h) = ((anchor.row, anchor.col), (head.row, head.col));
                let (lo_p, hi_p) = if a <= h { (a, h) } else { (h, a) };
                let (bs, be) = {
                    let b = buf(app).unwrap();
                    let bs = b.point_to_byte(Point {
                        row: lo_p.0,
                        col: lo_p.1,
                    });
                    let mut be = b.point_to_byte(Point {
                        row: hi_p.0,
                        col: hi_p.1,
                    });
                    // inclusive of head in vim
                    be = (be + 1).min(b.len_bytes());
                    (bs, be)
                };
                Self::apply_byte_range_op(app, op, bs, be);
            }
        }
    }

    fn dot_repeat(app: &mut App) {
        let lc = app.last_change.clone();
        if lc.kind == "insert" && !lc.inserted.is_empty() {
            Self::insert_str(app, &lc.inserted);
            return;
        }
        // Replay operator+motion (e.g. dw, ciw, dd) by re-feeding the keys.
        if let Some(opc) = lc.operator {
            // Save current last_change so the replay doesn't recursively
            // overwrite it indefinitely (we still let it update once).
            let saved = app.last_change.clone();
            KeyHandler::handle(app, Key::Char(opc));
            for ch in &lc.motion {
                KeyHandler::handle(app, Key::Char(*ch));
            }
            // Restore so consecutive `.` keep replaying the same change.
            app.last_change = saved;
        }
    }

    /// Insert a bracketed-paste payload at the cursor.
    pub fn paste(app: &mut App, s: &str) {
        Self::insert_str(app, s);
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
    fn visual_char_delete() {
        let mut app = app_with("hello world");
        KeyHandler::handle(&mut app, Key::Char('v'));
        for _ in 0..4 {
            KeyHandler::handle(&mut app, Key::Char('l'));
        }
        KeyHandler::handle(&mut app, Key::Char('d'));
        assert_eq!(app.buffers[0].rope.to_string(), " world");
        assert_eq!(app.mode, EditorMode::Normal);
    }

    #[test]
    fn visual_char_change() {
        let mut app = app_with("hello world");
        KeyHandler::handle(&mut app, Key::Char('v'));
        for _ in 0..4 {
            KeyHandler::handle(&mut app, Key::Char('l'));
        }
        KeyHandler::handle(&mut app, Key::Char('c'));
        assert_eq!(app.buffers[0].rope.to_string(), " world");
        assert_eq!(app.mode, EditorMode::Insert);
    }

    #[test]
    fn visual_char_yank() {
        let mut app = app_with("hello world");
        KeyHandler::handle(&mut app, Key::Char('v'));
        for _ in 0..4 {
            KeyHandler::handle(&mut app, Key::Char('l'));
        }
        KeyHandler::handle(&mut app, Key::Char('y'));
        assert_eq!(app.buffers[0].rope.to_string(), "hello world");
        assert_eq!(app.buffers[0].registers.get('"').unwrap().text, "hello");
        assert_eq!(app.mode, EditorMode::Normal);
    }

    #[test]
    fn mark_set_and_jump() {
        let mut app = app_with("line1\nline2\nline3\n");
        KeyHandler::handle(&mut app, Key::Char('j'));
        KeyHandler::handle(&mut app, Key::Char('m'));
        KeyHandler::handle(&mut app, Key::Char('a'));
        KeyHandler::handle(&mut app, Key::Char('g'));
        KeyHandler::handle(&mut app, Key::Char('g'));
        assert_eq!(cursor_of(&app).row, 0);
        KeyHandler::handle(&mut app, Key::Char('\''));
        KeyHandler::handle(&mut app, Key::Char('a'));
        assert_eq!(cursor_of(&app).row, 1);
    }

    #[test]
    fn semicolon_repeats_find() {
        let mut app = app_with("a.b.c.d");
        KeyHandler::handle(&mut app, Key::Char('f'));
        KeyHandler::handle(&mut app, Key::Char('.'));
        assert_eq!(cursor_of(&app).col, 1);
        KeyHandler::handle(&mut app, Key::Char(';'));
        assert_eq!(cursor_of(&app).col, 3);
    }

    #[test]
    fn dot_repeat_dw() {
        let mut app = app_with("foo bar baz qux");
        KeyHandler::handle(&mut app, Key::Char('d'));
        KeyHandler::handle(&mut app, Key::Char('w'));
        assert_eq!(app.buffers[0].rope.to_string(), "bar baz qux");
        KeyHandler::handle(&mut app, Key::Char('.'));
        assert_eq!(app.buffers[0].rope.to_string(), "baz qux");
    }

    #[test]
    fn search_n_jumps() {
        let mut app = app_with("foo bar foo");
        app.search.set("foo", true);
        KeyHandler::handle(&mut app, Key::Char('n'));
        assert_eq!(cursor_of(&app).col, 8);
    }
}
