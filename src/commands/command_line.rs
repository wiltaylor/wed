//! `:` command-line state and parser.

use crate::commands::{CommandContext, CommandRegistry, CommandResult};

#[derive(Debug, Default, Clone)]
pub struct CommandLineState {
    pub input: String,
    pub cursor: usize,
    pub history: Vec<String>,
    pub history_pos: Option<usize>,
    pub completions: Vec<String>,
    pub completion_idx: Option<usize>,
}

impl CommandLineState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.history_pos = None;
        self.completions.clear();
        self.completion_idx = None;
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.completions.clear();
        self.completion_idx = None;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // Remove the previous char (assuming ASCII for simplicity).
        let new_cursor = self.input[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.input.replace_range(new_cursor..self.cursor, "");
        self.cursor = new_cursor;
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let new = self.input[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.cursor = new;
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.input.len() {
            return;
        }
        if let Some((_, c)) = self.input[self.cursor..].char_indices().next() {
            self.cursor += c.len_utf8();
        }
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_pos {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(n) => n - 1,
        };
        self.history_pos = Some(idx);
        self.input = self.history[idx].clone();
        self.cursor = self.input.len();
    }

    pub fn history_next(&mut self) {
        match self.history_pos {
            None => {}
            Some(n) if n + 1 >= self.history.len() => {
                self.history_pos = None;
                self.input.clear();
                self.cursor = 0;
            }
            Some(n) => {
                self.history_pos = Some(n + 1);
                self.input = self.history[n + 1].clone();
                self.cursor = self.input.len();
            }
        }
    }

    pub fn complete(&mut self, registry: &CommandRegistry) {
        if self.completions.is_empty() {
            // Compute completions over the leading word (command name).
            let word: String = self
                .input
                .chars()
                .take_while(|c| !c.is_whitespace())
                .collect();
            self.completions = registry.complete(&word);
            if self.completions.is_empty() {
                return;
            }
            self.completion_idx = Some(0);
        } else if let Some(i) = self.completion_idx {
            self.completion_idx = Some((i + 1) % self.completions.len());
        }
        if let Some(i) = self.completion_idx {
            // Replace the leading word with the completion.
            let rest_start = self
                .input
                .find(char::is_whitespace)
                .unwrap_or(self.input.len());
            let rest = self.input[rest_start..].to_string();
            self.input = format!("{}{}", self.completions[i], rest);
            self.cursor = self.input.len();
        }
    }

    /// Parse the current input and execute it.
    pub fn accept(
        &mut self,
        registry: &CommandRegistry,
        ctx: &mut CommandContext,
    ) -> CommandResult {
        let line = std::mem::take(&mut self.input);
        self.cursor = 0;
        self.completions.clear();
        self.completion_idx = None;
        self.history.push(line.clone());
        self.history_pos = None;
        let parsed = parse_command_line(&line)?;
        let arg_refs: Vec<&str> = parsed.args.iter().map(|s| s.as_str()).collect();
        registry.invoke(&parsed.command, ctx, &arg_refs)
    }
}

/// A parsed command-line invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub command: String,
    pub args: Vec<String>,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Range {
    Whole,         // `%`
    Current,       // `.`
    Line(i64),     // numeric absolute
    Relative(i64), // `.,+5` second part etc.
    Pair(Box<Range>, Box<Range>),
}

/// Parse a `:`-line. The leading `:` may or may not be present. Handles
/// vim aliases, numeric line addresses, and `s` / `%s` substitutes.
pub fn parse_command_line(line: &str) -> anyhow::Result<ParsedCommand> {
    let line = line.trim_start_matches(':').trim();
    if line.is_empty() {
        anyhow::bail!("empty command");
    }

    // Numeric line jump: `:42`
    if let Ok(n) = line.parse::<u64>() {
        return Ok(ParsedCommand {
            command: "cursor.goto_line".into(),
            args: vec![n.to_string()],
            range: None,
        });
    }

    // Substitute: optional range prefix then `s/.../.../flags`
    if let Some(sub) = parse_substitute(line)? {
        return Ok(sub);
    }

    // Tokenize on whitespace
    let mut parts = line.split_whitespace();
    let head = parts.next().unwrap();
    let rest: Vec<String> = parts.map(|s| s.to_string()).collect();

    let (cmd, mut extra_args) = expand_alias(head);
    extra_args.extend(rest);
    Ok(ParsedCommand {
        command: cmd.to_string(),
        args: extra_args,
        range: None,
    })
}

/// Map vim aliases to canonical command names. Returns the canonical
/// name plus any leading args injected by the alias.
fn expand_alias(head: &str) -> (&'static str, Vec<String>) {
    match head {
        "w" | "write" => ("buffer.save", vec![]),
        "q" | "quit" => ("app.quit", vec![]),
        "q!" => ("app.quit", vec!["force".into()]),
        "wq" | "x" => ("app.write_quit", vec![]),
        "qa" | "qall" => ("app.quit_all", vec![]),
        "e" | "edit" => ("buffer.open", vec![]),
        "b" | "buffer" => ("buffer.goto", vec![]),
        "split" | "sp" => ("view.split_horizontal", vec![]),
        "vsplit" | "vsp" => ("view.split_vertical", vec![]),
        "tabnew" => ("tab.new", vec![]),
        "tabn" | "tabnext" => ("tab.next", vec![]),
        "tabp" | "tabprev" | "tabprevious" => ("tab.prev", vec![]),
        "close" => ("view.close", vec![]),
        // Pass through unknown command names verbatim. We need a 'static
        // str so we leak; this only happens for non-alias names that the
        // user types directly (and they typically map to existing
        // commands like `app.quit`).
        other => (Box::leak(other.to_string().into_boxed_str()), vec![]),
    }
}

/// Try to parse `s/pat/repl/flags` or `%s/pat/repl/flags` or
/// `.,+5s/pat/repl/flags` etc. Returns Ok(Some(parsed)) on a match,
/// Ok(None) if not a substitute.
fn parse_substitute(line: &str) -> anyhow::Result<Option<ParsedCommand>> {
    // Find an `s` followed by a delimiter (typically `/`)
    let bytes = line.as_bytes();
    // Range prefix
    let (range, rest_idx) = parse_range_prefix(line);
    let rest = &line[rest_idx..];
    if !rest.starts_with('s') {
        return Ok(None);
    }
    let after_s = &rest[1..];
    let delim = after_s.chars().next();
    let Some(delim) = delim else {
        return Ok(None);
    };
    if !"/#|,!".contains(delim) {
        return Ok(None);
    }
    // Split on delim. Allow escaped delimiter? Keep simple for now.
    let body = &after_s[delim.len_utf8()..];
    let parts: Vec<&str> = body.splitn(3, delim).collect();
    if parts.len() < 2 {
        return Ok(None);
    }
    let pat = parts[0].to_string();
    let repl = parts[1].to_string();
    let flags = parts.get(2).copied().unwrap_or("").to_string();
    let _ = bytes;
    let range_str = match range {
        Some(Range::Whole) => "%".to_string(),
        Some(Range::Current) => ".".to_string(),
        _ => "".to_string(),
    };
    Ok(Some(ParsedCommand {
        command: "search.substitute".into(),
        args: vec![range_str, pat, repl, flags],
        range,
    }))
}

/// Parse an optional leading range prefix. Returns the range (if any)
/// and the byte index where the rest of the line starts.
pub fn parse_range_prefix(line: &str) -> (Option<Range>, usize) {
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return (None, 0);
    }
    // `%`
    if bytes[0] == b'%' {
        return (Some(Range::Whole), 1);
    }
    // `.` (possibly followed by `,+N`)
    if bytes[0] == b'.' {
        // `.,+5` style
        if bytes.len() >= 2 && bytes[1] == b',' {
            // Find next non-numeric/sign char
            let mut i = 2;
            if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let off: i64 = line[2..i].parse().unwrap_or(0);
            return (
                Some(Range::Pair(
                    Box::new(Range::Current),
                    Box::new(Range::Relative(off)),
                )),
                i,
            );
        }
        return (Some(Range::Current), 1);
    }
    (None, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_w() {
        let p = parse_command_line(":w").unwrap();
        assert_eq!(p.command, "buffer.save");
    }

    #[test]
    fn parse_wq() {
        let p = parse_command_line(":wq").unwrap();
        assert_eq!(p.command, "app.write_quit");
    }

    #[test]
    fn parse_open_file() {
        let p = parse_command_line(":e file.txt").unwrap();
        assert_eq!(p.command, "buffer.open");
        assert_eq!(p.args, vec!["file.txt".to_string()]);
    }

    #[test]
    fn parse_line_number() {
        let p = parse_command_line(":42").unwrap();
        assert_eq!(p.command, "cursor.goto_line");
        assert_eq!(p.args, vec!["42".to_string()]);
    }

    #[test]
    fn parse_substitute_whole() {
        let p = parse_command_line(":%s/foo/bar/g").unwrap();
        assert_eq!(p.command, "search.substitute");
        assert_eq!(p.args[0], "%");
        assert_eq!(p.args[1], "foo");
        assert_eq!(p.args[2], "bar");
        assert_eq!(p.args[3], "g");
    }

    #[test]
    fn parse_substitute_current() {
        let p = parse_command_line(":s/foo/bar/").unwrap();
        assert_eq!(p.command, "search.substitute");
        assert_eq!(p.args[1], "foo");
        assert_eq!(p.args[2], "bar");
    }

    #[test]
    fn line_state_basic() {
        let mut s = CommandLineState::new();
        s.insert_char('w');
        s.insert_char('q');
        assert_eq!(s.input, "wq");
        s.backspace();
        assert_eq!(s.input, "w");
    }
}
