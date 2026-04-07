//! Command registry and dispatch.
//!
//! Commands are name → boxed closure mappings that take a `CommandContext`
//! and a slice of string args. The registry is the canonical place to add
//! built-in and user commands.

pub mod command_line;
pub mod context;
pub mod definitions;

pub use command_line::CommandLineState;
pub use context::CommandContext;

use std::collections::HashMap;

pub type CommandResult = anyhow::Result<()>;
pub type CommandFn = Box<dyn Fn(&mut CommandContext, &[&str]) -> CommandResult + Send + Sync>;

#[derive(Default)]
pub struct CommandRegistry {
    commands: HashMap<String, CommandFn>,
}

impl CommandRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&mut CommandContext, &[&str]) -> CommandResult + Send + Sync + 'static,
    {
        self.commands.insert(name.into(), Box::new(f));
    }

    pub fn invoke(&self, name: &str, ctx: &mut CommandContext, args: &[&str]) -> CommandResult {
        match self.commands.get(name) {
            Some(f) => f(ctx, args),
            None => Err(anyhow::anyhow!("unknown command: {name}")),
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn complete(&self, prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .commands
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        out.sort();
        out
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.commands.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_invoke_complete() {
        let mut reg = CommandRegistry::new();
        reg.register("app.quit", |ctx, _| {
            *ctx.quit = true;
            Ok(())
        });
        reg.register("app.write_quit", |_ctx, _| Ok(()));
        reg.register("buffer.save", |_ctx, _| Ok(()));

        assert!(reg.contains("app.quit"));
        let mut quit = false;
        let mut ctx = context::test_ctx(&mut quit);
        reg.invoke("app.quit", &mut ctx, &[]).unwrap();
        assert!(quit);

        let comps = reg.complete("app.");
        assert_eq!(comps, vec!["app.quit".to_string(), "app.write_quit".to_string()]);
    }
}
