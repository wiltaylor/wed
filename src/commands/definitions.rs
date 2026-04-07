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
}

pub fn register_app_commands(reg: &mut CommandRegistry) {
    reg.register("app.quit", |ctx, _| { *ctx.quit = true; Ok(()) });
    reg.register("app.quit_all", |ctx, _| { *ctx.quit = true; Ok(()) });
    reg.register("app.write_quit", |ctx, _| { *ctx.quit = true; Ok(()) });
    reg.register("app.reload_config", |_ctx, _| Ok(()));
    reg.register("app.open_config", |_ctx, _| Ok(()));
    reg.register("buffer.save", |_ctx, _| Ok(()));
    reg.register("buffer.open", |_ctx, _| Ok(()));
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
    for name in ["tab.new", "tab.close", "tab.next", "tab.prev", "tab.goto"] {
        reg.register(name, |_ctx, _| Ok(()));
    }
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
