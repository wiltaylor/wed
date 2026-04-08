// Built-in command definitions.
//
// Other agents add their own `register_*` functions here additively.

use crate::commands::CommandRegistry;

/// Register fuzzy picker / project search commands.
///
/// These are no-op stubs for now: the integration step will wire them to
/// actually open the corresponding picker / search overlay on `App`. Each
/// command body simply succeeds so that the registry contains them.
/// Convenience: register every group known so far. Other agents may
/// add their own `register_*` calls here as they land.
pub fn register_all(reg: &mut CommandRegistry) {
    register_app_commands(reg);
    register_view_commands(reg);
    register_search_commands(reg);
    register_tab_commands(reg);
    register_sidebar_commands(reg);
    register_picker_commands(reg);
    register_editor_commands(reg);
    register_git_commands(reg);
    register_dap_commands(reg);
    register_annotation_commands(reg);
}

/// Annotation commands. Registry bodies are stubs; the real dispatch
/// happens in `KeyHandler` (command-line path) where it has access to
/// the full `App` + annotation store.
pub fn register_annotation_commands(reg: &mut CommandRegistry) {
    for name in [
        "annotation.add",
        "annotation.remove",
        "annotation.prompt",
        "annotation.list",
    ] {
        reg.register(name, |_ctx, _| Ok(()));
    }
}

/// DAP commands. Bodies are stubs at the registry layer; the real
/// dispatch lives in `KeyHandler::run_leader_command` which mutates
/// `App.dap` and bottom-panel panes directly.
pub fn register_dap_commands(reg: &mut CommandRegistry) {
    for name in [
        "dap.breakpoint.toggle",
        "dap.launch",
        "dap.stop",
        "dap.continue",
        "dap.step_over",
        "dap.step_into",
        "dap.step_out",
        "dap.pause",
        "dap.panel.toggle",
    ] {
        reg.register(name, |_ctx, _| Ok(()));
    }
}

/// Git commands. Their bodies are stubs at the registry layer; the
/// real work is dispatched in `KeyHandler::run_leader_command` so it
/// can mutate `App.git` and the file/commit panes directly.
pub fn register_git_commands(reg: &mut CommandRegistry) {
    for name in [
        "git.stage",
        "git.unstage",
        "git.delete",
        "git.commit",
        "git.file_history",
        "panel.commit",
    ] {
        reg.register(name, |_ctx, _| Ok(()));
    }
}

/// Register editor motion / edit / mode commands.
///
/// These are intentionally lightweight: they take whatever they need from
/// `CommandContext` (buffers + mode + quit). The richer dispatch lives in
/// `crate::input::key_handler::KeyHandler::handle`.
pub fn register_editor_commands(reg: &mut CommandRegistry) {
    use crate::editor::motions;
    use crate::editor::Cursor;

    // Cursor motions — these operate on buffer 0 with a (0,0) cursor stub
    // since CommandContext has no view/cursor handle yet. The KeyHandler
    // is the canonical path; these exist so `:`-commands and config
    // bindings can resolve them.
    macro_rules! cmd {
        ($name:literal, $f:expr) => {
            reg.register($name, $f);
        };
    }
    cmd!("cursor.move_left", |_ctx, _| Ok(()));
    cmd!("cursor.move_right", |_ctx, _| Ok(()));
    cmd!("cursor.move_up", |_ctx, _| Ok(()));
    cmd!("cursor.move_down", |_ctx, _| Ok(()));
    cmd!("cursor.word_forward", |_ctx, _| Ok(()));
    cmd!("cursor.word_backward", |_ctx, _| Ok(()));
    cmd!("cursor.line_start", |_ctx, _| Ok(()));
    cmd!("cursor.line_end", |_ctx, _| Ok(()));
    cmd!("cursor.buffer_top", |_ctx, _| Ok(()));
    cmd!("cursor.buffer_bottom", |_ctx, _| Ok(()));
    // `cursor.goto_line` is registered by `register_app_commands` already.

    cmd!("edit.undo", |ctx, _| {
        if let Some(b) = ctx.buffers.first_mut() {
            b.undo();
        }
        Ok(())
    });
    cmd!("edit.redo", |ctx, _| {
        if let Some(b) = ctx.buffers.first_mut() {
            b.redo();
        }
        Ok(())
    });
    cmd!("edit.yank", |_ctx, _| Ok(()));
    cmd!("edit.paste_after", |_ctx, _| Ok(()));
    cmd!("edit.paste_before", |_ctx, _| Ok(()));
    cmd!("edit.delete_line", |ctx, _| {
        if let Some(b) = ctx.buffers.first_mut() {
            if b.line_count() > 0 {
                use crate::editor::buffer::Point;
                let from = b.point_to_byte(Point { row: 0, col: 0 });
                let to = if b.line_count() > 1 {
                    b.point_to_byte(Point { row: 1, col: 0 })
                } else {
                    b.len_bytes()
                };
                b.delete(from..to);
            }
        }
        Ok(())
    });
    cmd!("edit.change_line", |_ctx, _| Ok(()));
    cmd!("edit.indent", |_ctx, _| Ok(()));
    cmd!("edit.dedent", |_ctx, _| Ok(()));
    cmd!("edit.comment_toggle", |ctx, _| {
        if let Some(b) = ctx.buffers.first_mut() {
            let cs = crate::editor::ops::comment_string_for(b.language_id.as_deref());
            crate::editor::ops::comment_toggle_rows(b, 0..=0, cs);
        }
        Ok(())
    });

    cmd!("mode.normal", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Normal;
        Ok(())
    });
    cmd!("mode.insert", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Insert;
        Ok(())
    });
    cmd!("mode.insert_line_start", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Insert;
        Ok(())
    });
    cmd!("mode.append", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Insert;
        Ok(())
    });
    cmd!("mode.append_line_end", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Insert;
        Ok(())
    });
    cmd!("mode.open_below", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Insert;
        Ok(())
    });
    cmd!("mode.open_above", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Insert;
        Ok(())
    });
    cmd!("mode.visual", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Visual(crate::input::mode::VisualKind::Char);
        Ok(())
    });
    cmd!("mode.visual_line", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Visual(crate::input::mode::VisualKind::Line);
        Ok(())
    });
    cmd!("mode.visual_block", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Visual(crate::input::mode::VisualKind::Block);
        Ok(())
    });
    cmd!("mode.replace", |ctx, _| {
        *ctx.mode = crate::input::EditorMode::Replace;
        Ok(())
    });

    // buffer.save / open already registered by register_app_commands.
    cmd!("buffer.save_all", |ctx, _| {
        for b in ctx.buffers.iter_mut() {
            let _ = b.save();
        }
        Ok(())
    });
    cmd!("buffer.close", |_ctx, _| Ok(()));
    cmd!("buffer.new", |ctx, _| {
        ctx.buffers.push(crate::editor::Buffer::default());
        Ok(())
    });
    cmd!("buffer.next", |_ctx, _| Ok(()));
    cmd!("buffer.prev", |_ctx, _| Ok(()));

    let _ = motions::left; // keep `motions` referenced for lints
    let _ = Cursor::default();
}

pub fn register_app_commands(reg: &mut CommandRegistry) {
    reg.register("app.quit", |ctx, args| {
        let force = args.iter().any(|a| *a == "force");
        if !force {
            if let Some(b) = ctx.buffers.iter().find(|b| b.dirty) {
                let name = b
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "[No Name]".to_string());
                anyhow::bail!("unsaved changes in {name} (use :q! to force)");
            }
        }
        *ctx.quit = true;
        Ok(())
    });
    reg.register("app.quit_all", |ctx, args| {
        let force = args.iter().any(|a| *a == "force");
        if !force {
            if let Some(b) = ctx.buffers.iter().find(|b| b.dirty) {
                let name = b
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "[No Name]".to_string());
                anyhow::bail!("unsaved changes in {name} (use :qa! to force)");
            }
        }
        *ctx.quit = true;
        Ok(())
    });
    reg.register("app.write_quit", |ctx, _| {
        for b in ctx.buffers.iter_mut() {
            if b.path.is_some() {
                b.save()?;
            }
        }
        *ctx.quit = true;
        Ok(())
    });
    reg.register("app.reload_config", |_ctx, _| Ok(()));
    reg.register("app.open_config", |_ctx, _| Ok(()));
    reg.register("buffer.save", |ctx, _| {
        if let Some(b) = ctx.buffers.first_mut() {
            b.save()?;
        }
        Ok(())
    });
    reg.register("buffer.open", |ctx, args| {
        if let Some(path) = args.first() {
            let buf = crate::editor::Buffer::from_path(path)?;
            ctx.buffers.push(buf);
            // Point active view at the new buffer.
            let new_idx = ctx.buffers.len() - 1;
            if let Some(tab) = ctx.layout.active_tab_mut() {
                let id = tab.active_view;
                if let Some(view) = tab.root.find_mut(id) {
                    view.buffer_id = crate::app::BufferId(new_idx as u64);
                    view.cursor = (0, 0);
                }
            }
        }
        Ok(())
    });
    reg.register("buffer.goto", |_ctx, _| Ok(()));
    reg.register("buffer.list", |_ctx, _| Ok(()));
    reg.register("cursor.goto_line", |_ctx, _| Ok(()));
}

pub fn register_view_commands(reg: &mut CommandRegistry) {
    for name in [
        "view.split_horizontal",
        "view.split_vertical",
        "view.close",
        "view.focus_left",
        "view.focus_right",
        "view.focus_up",
        "view.focus_down",
        "view.resize_wider",
        "view.resize_narrower",
        "view.zoom",
    ] {
        reg.register(name, |_ctx, _| Ok(()));
    }
}

pub fn register_search_commands(reg: &mut CommandRegistry) {
    for name in [
        "search.forward",
        "search.backward",
        "search.next",
        "search.prev",
        "search.clear_highlight",
        "search.replace",
        "search.substitute",
        "search.files",
    ] {
        reg.register(name, |_ctx, _| Ok(()));
    }
    // NOTE: `search.project` is registered by `register_picker_commands`.
}

pub fn register_tab_commands(reg: &mut CommandRegistry) {
    reg.register("tab.next", |ctx, _| {
        let n = ctx.layout.tabs.len();
        if n > 0 {
            ctx.layout.active_tab = (ctx.layout.active_tab + 1) % n;
        }
        Ok(())
    });
    reg.register("tab.prev", |ctx, _| {
        let n = ctx.layout.tabs.len();
        if n > 0 {
            ctx.layout.active_tab = (ctx.layout.active_tab + n - 1) % n;
        }
        Ok(())
    });
    reg.register("tab.close", |ctx, _| {
        let n = ctx.layout.tabs.len();
        if n > 1 {
            let i = ctx.layout.active_tab;
            ctx.layout.tabs.remove(i);
            if ctx.layout.active_tab >= ctx.layout.tabs.len() {
                ctx.layout.active_tab = ctx.layout.tabs.len() - 1;
            }
        }
        Ok(())
    });
    reg.register("tab.new", |_ctx, _| Ok(()));
    reg.register("tab.goto", |_ctx, _| Ok(()));
}

pub fn register_sidebar_commands(reg: &mut CommandRegistry) {
    for name in [
        "sidebar.left_toggle",
        "sidebar.right_toggle",
        "sidebar.focus",
        "sidebar.pane",
    ] {
        reg.register(name, |_ctx, _| Ok(()));
    }
}

pub fn register_picker_commands(reg: &mut CommandRegistry) {
    reg.register("picker.files", |_ctx, _args| Ok(()));
    reg.register("picker.buffers", |_ctx, _args| Ok(()));
    reg.register("picker.symbols", |_ctx, _args| Ok(()));
    reg.register("picker.commands", |_ctx, _args| Ok(()));
    reg.register("picker.git_files", |_ctx, _args| Ok(()));
    reg.register("picker.diagnostics", |_ctx, _args| Ok(()));
    reg.register("search.project", |_ctx, _args| Ok(()));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registers_all_picker_commands() {
        let mut reg = CommandRegistry::new();
        register_picker_commands(&mut reg);
        for name in [
            "picker.files",
            "picker.buffers",
            "picker.symbols",
            "picker.commands",
            "picker.git_files",
            "picker.diagnostics",
            "search.project",
        ] {
            assert!(reg.contains(name), "missing {name}");
        }
    }
}
