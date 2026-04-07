//! Keybinding configuration: parsing key strings, building a KeyTrie,
//! and resolving multi-key sequences per editor mode.

use crate::input::keys::Key;
use crate::input::EditorMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A bound action: a command name plus optional positional args.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCommand {
    pub command: String,
    pub args: Vec<String>,
}

impl BoundCommand {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
        }
    }
    pub fn with_args(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
        }
    }
}

/// A trie of normalized keys → bound commands. Internal nodes are partial
/// matches, leaves are full matches.
#[derive(Debug, Clone, Default)]
pub struct KeyTrie {
    pub children: HashMap<Key, KeyTrie>,
    pub command: Option<BoundCommand>,
}

impl KeyTrie {
    pub fn insert(&mut self, keys: &[Key], cmd: BoundCommand) {
        if keys.is_empty() {
            self.command = Some(cmd);
            return;
        }
        self.children
            .entry(keys[0])
            .or_default()
            .insert(&keys[1..], cmd);
    }

    /// Walk the trie by the given key sequence and return the node at
    /// the end, or None if the path doesn't exist.
    pub fn get_node(&self, keys: &[Key]) -> Option<&KeyTrie> {
        let mut node = self;
        for k in keys {
            node = node.children.get(k)?;
        }
        Some(node)
    }

    pub fn get(&self, keys: &[Key]) -> Resolution {
        if keys.is_empty() {
            return if self.command.is_some() {
                Resolution::Match(self.command.clone().unwrap())
            } else if !self.children.is_empty() {
                Resolution::Pending
            } else {
                Resolution::NoMatch
            };
        }
        match self.children.get(&keys[0]) {
            Some(child) => child.get(&keys[1..]),
            None => Resolution::NoMatch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Pending,
    Match(BoundCommand),
    NoMatch,
}

/// A keybinding configuration: per-mode trie plus a leader trie.
#[derive(Debug, Clone, Default)]
pub struct Keybindings {
    pub per_mode: HashMap<ModeKey, KeyTrie>,
    pub leader: KeyTrie,
    pub leader_key: Key,
}

/// Hashable mode key (EditorMode is Copy/Eq but Visual carries data; we
/// project to a coarser tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeKey {
    Normal,
    Insert,
    Visual,
    Command,
    Search,
    Replace,
}

impl ModeKey {
    pub fn from_mode(m: EditorMode) -> Self {
        match m {
            EditorMode::Normal | EditorMode::Pending(_) | EditorMode::Operator(_) => {
                ModeKey::Normal
            }
            EditorMode::Insert => ModeKey::Insert,
            EditorMode::Visual(_) => ModeKey::Visual,
            EditorMode::Command => ModeKey::Command,
            EditorMode::Search => ModeKey::Search,
            EditorMode::Replace => ModeKey::Replace,
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "normal" => ModeKey::Normal,
            "insert" => ModeKey::Insert,
            "visual" => ModeKey::Visual,
            "command" => ModeKey::Command,
            "search" => ModeKey::Search,
            "replace" => ModeKey::Replace,
            _ => return None,
        })
    }
}

impl Keybindings {
    pub fn new() -> Self {
        Self {
            per_mode: HashMap::new(),
            leader: KeyTrie::default(),
            leader_key: Key::Char(' '),
        }
    }

    pub fn bind(&mut self, mode: ModeKey, keys_str: &str, cmd: BoundCommand) -> Result<(), String> {
        let keys = parse_key_sequence(keys_str, self.leader_key)?;
        // <leader>... bindings: store under leader trie too for clarity.
        self.per_mode.entry(mode).or_default().insert(&keys, cmd);
        Ok(())
    }

    pub fn bind_leader(&mut self, keys_str: &str, cmd: BoundCommand) -> Result<(), String> {
        let keys = parse_key_sequence(keys_str, self.leader_key)?;
        self.leader.insert(&keys, cmd.clone());
        // Also bind in normal mode as <leader>...
        let mut full = vec![self.leader_key];
        full.extend(keys);
        self.per_mode
            .entry(ModeKey::Normal)
            .or_default()
            .insert(&full, cmd);
        Ok(())
    }

    /// Return the normal-mode trie subtree under `[leader_key, ...keys]`.
    pub fn leader_trie_at(&self, keys: &[Key]) -> Option<&KeyTrie> {
        let trie = self.per_mode.get(&ModeKey::Normal)?;
        let mut path = vec![self.leader_key];
        path.extend_from_slice(keys);
        trie.get_node(&path)
    }

    pub fn resolve(&self, mode: EditorMode, keys: &[Key]) -> Resolution {
        let mk = ModeKey::from_mode(mode);
        match self.per_mode.get(&mk) {
            Some(trie) => trie.get(keys),
            None => Resolution::NoMatch,
        }
    }

    /// Built-in default keybindings (subset of the spec WCL example).
    pub fn defaults() -> Self {
        let mut kb = Self::new();

        // Normal mode
        let normal: &[(&str, &str)] = &[
            ("h", "cursor.left"),
            ("j", "cursor.down"),
            ("k", "cursor.up"),
            ("l", "cursor.right"),
            ("w", "cursor.word_next"),
            ("b", "cursor.word_prev"),
            ("0", "cursor.line_start"),
            ("$", "cursor.line_end"),
            ("gg", "cursor.buffer_start"),
            ("G", "cursor.buffer_end"),
            ("i", "mode.insert"),
            ("a", "mode.insert_after"),
            ("o", "edit.open_below"),
            ("O", "edit.open_above"),
            ("v", "mode.visual_char"),
            ("V", "mode.visual_line"),
            ("u", "edit.undo"),
            ("ctrl-r", "edit.redo"),
            ("dd", "edit.delete_line"),
            ("yy", "edit.yank_line"),
            ("p", "edit.paste_after"),
            ("P", "edit.paste_before"),
            ("/", "search.forward"),
            ("?", "search.backward"),
            ("n", "search.next"),
            ("N", "search.prev"),
            (":", "command.open"),
            ("ctrl-w-v", "view.split_vertical"),
            ("ctrl-w-s", "view.split_horizontal"),
            ("ctrl-w-h", "view.focus_left"),
            ("ctrl-w-j", "view.focus_down"),
            ("ctrl-w-k", "view.focus_up"),
            ("ctrl-w-l", "view.focus_right"),
            ("ctrl-w-q", "view.close"),
            ("escape", "mode.normal"),
        ];
        for (k, c) in normal {
            kb.bind(ModeKey::Normal, k, BoundCommand::new(*c)).unwrap();
        }

        // Insert mode
        let insert: &[(&str, &str)] = &[
            ("escape", "mode.normal"),
            ("ctrl-c", "mode.normal"),
            ("ctrl-w", "edit.delete_word_back"),
        ];
        for (k, c) in insert {
            kb.bind(ModeKey::Insert, k, BoundCommand::new(*c)).unwrap();
        }

        // Visual mode
        let visual: &[(&str, &str)] = &[
            ("escape", "mode.normal"),
            ("y", "edit.yank"),
            ("d", "edit.delete"),
            ("c", "edit.change"),
            (">", "edit.indent"),
            ("<", "edit.dedent"),
        ];
        for (k, c) in visual {
            kb.bind(ModeKey::Visual, k, BoundCommand::new(*c)).unwrap();
        }

        // Leader bindings
        let leader: &[(&str, &str)] = &[
            ("ff", "search.files"),
            ("fg", "search.project"),
            ("fb", "buffer.list"),
            ("w", "buffer.save"),
            ("q", "app.quit"),
            ("e", "sidebar.left_toggle"),
            ("k", "lsp.hover"),
            ("gd", "lsp.definition"),
            ("gi", "lsp.implementation"),
            ("gr", "lsp.references"),
            ("x", "panel.toggle"),
            ("gc", "panel.commit"),
        ];
        for (k, c) in leader {
            kb.bind_leader(k, BoundCommand::new(*c)).unwrap();
        }

        kb
    }
}

/// Serializable form used by `Config` for TOML loading. The runtime
/// representation is `Keybindings` built via `into_runtime`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeybindingConfig {
    #[serde(default)]
    pub normal: HashMap<String, BindingValue>,
    #[serde(default)]
    pub insert: HashMap<String, BindingValue>,
    #[serde(default)]
    pub visual: HashMap<String, BindingValue>,
    #[serde(default)]
    pub command: HashMap<String, BindingValue>,
    #[serde(default)]
    pub search: HashMap<String, BindingValue>,
    #[serde(default)]
    pub replace: HashMap<String, BindingValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BindingValue {
    Simple(String),
    Detailed {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

impl BindingValue {
    pub fn into_bound(self) -> BoundCommand {
        match self {
            BindingValue::Simple(s) => BoundCommand::new(s),
            BindingValue::Detailed { command, args } => BoundCommand::with_args(command, args),
        }
    }
}

impl KeybindingConfig {
    pub fn merge_into(&self, kb: &mut Keybindings) -> Result<(), String> {
        for (mode, map) in [
            (ModeKey::Normal, &self.normal),
            (ModeKey::Insert, &self.insert),
            (ModeKey::Visual, &self.visual),
            (ModeKey::Command, &self.command),
            (ModeKey::Search, &self.search),
            (ModeKey::Replace, &self.replace),
        ] {
            for (k, v) in map {
                kb.bind(mode, k, v.clone().into_bound())?;
            }
        }
        Ok(())
    }
}

// =====================================================================
// Key string parser
// =====================================================================

/// Parse a key sequence like `"gg"`, `"ctrl-w-v"`, `"<leader>ff"`, `"escape"`,
/// `"F5"`, `"shift-F11"` into a vector of normalized `Key`s.
pub fn parse_key_sequence(s: &str, leader: Key) -> Result<Vec<Key>, String> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix("<leader>") {
            out.push(leader);
            rest = stripped;
            continue;
        }
        if rest.starts_with('<') {
            if let Some(end) = rest[1..].find('>') {
                let tok = &rest[1..1 + end];
                if let Ok(k) = parse_named_key(tok) {
                    out.push(k);
                    rest = &rest[2 + end..];
                    continue;
                }
            }
            out.push(Key::Char('<'));
            rest = &rest[1..];
            continue;
        }
        // Lookahead: a "ctrl-..." or "alt-..." or "shift-..." or "FNN" token
        // is delimited by next single char OR end. We try greedy: longest run
        // of `[a-zA-Z0-9-]` that contains a `-` or starts with F<digit> or
        // matches a named key.
        let token_end = next_token_end(rest);
        let tok = &rest[..token_end];
        if let Some(keys) = expand_token(tok) {
            out.extend(keys);
            rest = &rest[token_end..];
        } else {
            // Single char fallback
            let c = rest.chars().next().unwrap();
            out.push(Key::Char(c));
            rest = &rest[c.len_utf8()..];
        }
    }
    Ok(out)
}

fn next_token_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    // Modifier-chain: consume the entire run of `[A-Za-z0-9-]` so chords
    // like `ctrl-w-v` are taken as one token.
    for prefix in ["ctrl-", "alt-", "shift-"] {
        if s.starts_with(prefix) {
            let mut end = prefix.len();
            while end < bytes.len() {
                let b = bytes[end];
                if b.is_ascii_alphanumeric() || b == b'-' {
                    end += 1;
                } else {
                    break;
                }
            }
            return end;
        }
    }
    // Named key (escape, enter, tab, space, backspace, F1..F12)
    let named = [
        "escape",
        "enter",
        "tab",
        "space",
        "backspace",
        "delete",
        "insert",
        "home",
        "end",
        "pageup",
        "pagedown",
        "up",
        "down",
        "left",
        "right",
        "null",
    ];
    for n in named {
        if s.len() >= n.len() && s[..n.len()].eq_ignore_ascii_case(n) {
            return n.len();
        }
    }
    if bytes.len() >= 2 && (bytes[0] == b'F' || bytes[0] == b'f') && bytes[1].is_ascii_digit() {
        let mut end = 2;
        if bytes.len() > 2 && bytes[2].is_ascii_digit() {
            end = 3;
        }
        return end;
    }
    // Single char
    s.chars().next().map(|c| c.len_utf8()).unwrap_or(0)
}

/// Expand a token like `ctrl-w-v` into a sequence of keys (`Ctrl('w')`, `Char('v')`).
/// Returns None if the token cannot be parsed.
fn expand_token(tok: &str) -> Option<Vec<Key>> {
    if let Some(rest) = tok.strip_prefix("ctrl-") {
        // First segment after `ctrl-` is the modified key; any remainder
        // (joined by `-`) is a follow-up plain key sequence.
        let mut parts = rest.splitn(2, '-');
        let first = parts.next()?;
        if first.len() != 1 {
            return None;
        }
        let mut keys = vec![Key::Ctrl(
            first.chars().next().unwrap().to_ascii_lowercase(),
        )];
        if let Some(trailing) = parts.next() {
            // Recursively expand the trailing portion as its own sequence.
            // It might be `v`, `w-v`, `escape`, etc.
            let leader = Key::Char(' '); // unused for non-<leader> tokens
            let extra = parse_key_sequence(trailing, leader).ok()?;
            keys.extend(extra);
        }
        return Some(keys);
    }
    if let Some(rest) = tok.strip_prefix("alt-") {
        if rest.len() == 1 {
            return Some(vec![Key::Alt(rest.chars().next().unwrap())]);
        }
        return None;
    }
    if let Some(rest) = tok.strip_prefix("shift-") {
        if let Some(k) = parse_function_key(rest) {
            return Some(vec![k]);
        }
        if rest.len() == 1 {
            return Some(vec![Key::Char(
                rest.chars().next().unwrap().to_ascii_uppercase(),
            )]);
        }
        return None;
    }
    try_parse_token(tok).map(|k| vec![k])
}

fn try_parse_token(tok: &str) -> Option<Key> {
    // Modifier-chain
    if let Some(rest) = tok.strip_prefix("ctrl-") {
        // ctrl-X or ctrl-X-Y (chord like ctrl-w-v expands to two keys, but
        // here token is treated as a single key — the trie handles `ctrl-w`
        // then plain `v` separately. We'll treat ctrl-w-v as ctrl-w + v by
        // returning only the first key here and letting the caller add
        // the trailing piece.) — But our `parse_key_sequence` consumed the
        // whole chain as one token. Handle that by parsing recursively:
        // we accept the first segment as Ctrl(c) and ignore the rest if
        // there is none, otherwise we return None to signal "split".
        let mut parts = rest.splitn(2, '-');
        let first = parts.next()?;
        let _trailing = parts.next();
        if first.len() == 1 {
            return Some(Key::Ctrl(
                first.chars().next().unwrap().to_ascii_lowercase(),
            ));
        }
        return None;
    }
    if let Some(rest) = tok.strip_prefix("alt-") {
        if rest.len() == 1 {
            return Some(Key::Alt(rest.chars().next().unwrap()));
        }
        return None;
    }
    if let Some(rest) = tok.strip_prefix("shift-") {
        // shift-F11 → F(11), shift-X → Char(X uppercase)
        if let Some(n) = parse_function_key(rest) {
            return Some(n);
        }
        if rest.len() == 1 {
            return Some(Key::Char(rest.chars().next().unwrap().to_ascii_uppercase()));
        }
        return None;
    }
    if let Some(k) = parse_function_key(tok) {
        return Some(k);
    }
    Some(match tok.to_ascii_lowercase().as_str() {
        "escape" | "esc" => Key::Esc,
        "enter" | "return" | "cr" => Key::Enter,
        "tab" => Key::Tab,
        "space" => Key::Char(' '),
        "backspace" | "bs" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "insert" | "ins" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        _ => return None,
    })
}

fn parse_function_key(s: &str) -> Option<Key> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    if bytes[0] != b'F' && bytes[0] != b'f' {
        return None;
    }
    let n: u8 = s[1..].parse().ok()?;
    Some(Key::F(n))
}

fn parse_named_key(tok: &str) -> Result<Key, String> {
    try_parse_token(tok).ok_or_else(|| format!("unknown key token: {tok}"))
}

// The above try_parse_token returns None for `ctrl-w-v` chord; we need to
// expand chord tokens during parse_key_sequence. Override that here:
//
// We re-implement chord expansion explicitly in parse_key_sequence by
// short-circuiting modifier+'-'+modifier-key chains.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::keys::Key;

    #[test]
    fn parse_simple() {
        let leader = Key::Char(' ');
        assert_eq!(
            parse_key_sequence("gg", leader).unwrap(),
            vec![Key::Char('g'), Key::Char('g')]
        );
    }

    #[test]
    fn parse_escape() {
        let leader = Key::Char(' ');
        assert_eq!(
            parse_key_sequence("escape", leader).unwrap(),
            vec![Key::Esc]
        );
    }

    #[test]
    fn parse_function() {
        let leader = Key::Char(' ');
        assert_eq!(parse_key_sequence("F5", leader).unwrap(), vec![Key::F(5)]);
        assert_eq!(
            parse_key_sequence("shift-F11", leader).unwrap(),
            vec![Key::F(11)]
        );
    }

    #[test]
    fn parse_leader() {
        let leader = Key::Char(' ');
        assert_eq!(
            parse_key_sequence("<leader>ff", leader).unwrap(),
            vec![Key::Char(' '), Key::Char('f'), Key::Char('f')]
        );
    }

    #[test]
    fn parse_ctrl_chord() {
        let leader = Key::Char(' ');
        // ctrl-w-v should expand to ctrl-w then plain 'v'
        let ks = parse_key_sequence("ctrl-w-v", leader).unwrap();
        assert_eq!(ks, vec![Key::Ctrl('w'), Key::Char('v')]);
    }

    #[test]
    fn trie_pending_then_match() {
        let mut kb = Keybindings::new();
        kb.bind(
            ModeKey::Normal,
            "gg",
            BoundCommand::new("cursor.buffer_start"),
        )
        .unwrap();
        let g = vec![Key::Char('g')];
        let gg = vec![Key::Char('g'), Key::Char('g')];
        assert_eq!(kb.resolve(EditorMode::Normal, &g), Resolution::Pending);
        assert_eq!(
            kb.resolve(EditorMode::Normal, &gg),
            Resolution::Match(BoundCommand::new("cursor.buffer_start"))
        );
    }

    #[test]
    fn leader_expansion() {
        let kb = Keybindings::defaults();
        let space = Key::Char(' ');
        let seq = vec![space, Key::Char('q')];
        assert_eq!(
            kb.resolve(EditorMode::Normal, &seq),
            Resolution::Match(BoundCommand::new("app.quit"))
        );
    }
}
