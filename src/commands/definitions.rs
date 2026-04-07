// Built-in command definitions.
//
// Other agents add their own `register_*` functions here additively.

use crate::commands::CommandRegistry;

/// Register fuzzy picker / project search commands.
///
/// These are no-op stubs for now: the integration step will wire them to
/// actually open the corresponding picker / search overlay on `App`. Each
/// command body simply succeeds so that the registry contains them.
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
