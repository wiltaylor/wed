pub mod command_line;
pub mod context;
pub mod definitions;

pub use command_line::CommandLineState;
pub use context::CommandContext;

use std::collections::HashMap;

pub type CommandResult = anyhow::Result<()>;
pub type CommandFn = fn(&mut CommandContext) -> CommandResult;

#[derive(Default)]
pub struct CommandRegistry {
    pub commands: HashMap<String, CommandFn>,
}

impl CommandRegistry {
    pub fn new() -> Self { Self::default() }
    pub fn register(&mut self, name: impl Into<String>, f: CommandFn) {
        self.commands.insert(name.into(), f);
    }
}
