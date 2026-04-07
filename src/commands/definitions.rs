// Built-in command definitions.
//
// Other agents add their own `register_*` functions here additively.

use crate::commands::{CommandContext, CommandRegistry, CommandResult};

// ---------- picker commands ----------
//
// These are no-op stubs for now: the integration step will wire them to
// actually open the corresponding picker / search overlay on `App`. Each
// command body simply succeeds so that the registry contains them.

fn picker_files_cmd(_ctx: &mut CommandContext) -> CommandResult { Ok(()) }
fn picker_buffers_cmd(_ctx: &mut CommandContext) -> CommandResult { Ok(()) }
fn picker_symbols_cmd(_ctx: &mut CommandContext) -> CommandResult { Ok(()) }
fn picker_commands_cmd(_ctx: &mut CommandContext) -> CommandResult { Ok(()) }
fn picker_git_files_cmd(_ctx: &mut CommandContext) -> CommandResult { Ok(()) }
fn picker_diagnostics_cmd(_ctx: &mut CommandContext) -> CommandResult { Ok(()) }
fn search_project_cmd(_ctx: &mut CommandContext) -> CommandResult { Ok(()) }

/// Register fuzzy picker / project search commands.
pub fn register_picker_commands(reg: &mut CommandRegistry) {
    reg.register("picker.files", picker_files_cmd);
    reg.register("picker.buffers", picker_buffers_cmd);
    reg.register("picker.symbols", picker_symbols_cmd);
    reg.register("picker.commands", picker_commands_cmd);
    reg.register("picker.git_files", picker_git_files_cmd);
    reg.register("picker.diagnostics", picker_diagnostics_cmd);
    reg.register("search.project", search_project_cmd);
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
            assert!(reg.commands.contains_key(name), "missing {name}");
        }
    }
}
