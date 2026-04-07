pub mod keybindings;
pub mod schema;
pub mod theme;

pub use keybindings::{
    BindingValue, BoundCommand, KeyTrie, KeybindingConfig, Keybindings, ModeKey, Resolution,
};
pub use schema::{
    DapConfig, EditorConfig, FiletypeConfig, LspConfig, SearchConfig, TerminalConfig, UiConfig,
};
pub use theme::Theme;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub leader: String,
    pub editor: EditorConfig,
    pub ui: UiConfig,
    pub search: SearchConfig,
    pub terminal: TerminalConfig,
    pub theme: Theme,
    pub keybindings: KeybindingConfig,
    pub leader_bindings: HashMap<String, BindingValue>,
    pub lsp: HashMap<String, LspConfig>,
    pub dap: HashMap<String, DapConfig>,
    pub filetype: HashMap<String, FiletypeConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            leader: "<space>".into(),
            editor: EditorConfig::default(),
            ui: UiConfig::default(),
            search: SearchConfig::default(),
            terminal: TerminalConfig::default(),
            theme: Theme::default(),
            keybindings: KeybindingConfig::default(),
            leader_bindings: HashMap::new(),
            lsp: HashMap::new(),
            dap: HashMap::new(),
            filetype: HashMap::new(),
        }
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())?;
        let cfg: Config = toml::from_str(&text)?;
        Ok(cfg)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(text)?)
    }

    /// Build the runtime `Keybindings` by merging defaults, the user's
    /// `[keybindings.*]` table, and `[leader_bindings]`.
    pub fn build_keybindings(&self) -> Result<Keybindings, String> {
        let mut kb = Keybindings::defaults();
        // Override leader if set
        if let Ok(seq) = keybindings::parse_key_sequence(&self.leader, kb.leader_key) {
            if let Some(k) = seq.into_iter().next() {
                kb.leader_key = k;
            }
        }
        self.keybindings.merge_into(&mut kb)?;
        for (k, v) in &self.leader_bindings {
            kb.bind_leader(k, v.clone().into_bound())?;
        }
        Ok(kb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_roundtrip() {
        let cfg = Config::default();
        let s = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.editor.tab_width, cfg.editor.tab_width);
        assert_eq!(back.leader, cfg.leader);
    }

    #[test]
    fn parse_user_toml() {
        let text = r#"
leader = "<space>"

[editor]
tab_width = 2
expand_tabs = false

[ui]
tabline = false

[lsp.rust]
command = "rust-analyzer"
filetypes = ["rust"]

[keybindings.normal]
"jk" = "mode.normal"

[leader_bindings]
"ff" = "search.files"
"#;
        let cfg = Config::from_str(text).unwrap();
        assert_eq!(cfg.editor.tab_width, 2);
        assert!(!cfg.editor.expand_tabs);
        assert_eq!(cfg.lsp["rust"].command, "rust-analyzer");
        let kb = cfg.build_keybindings().unwrap();
        // jk should resolve in normal mode
        let seq = vec![
            crate::input::keys::Key::Char('j'),
            crate::input::keys::Key::Char('k'),
        ];
        match kb.resolve(crate::input::EditorMode::Normal, &seq) {
            Resolution::Match(c) => assert_eq!(c.command, "mode.normal"),
            other => panic!("expected match, got {other:?}"),
        }
    }
}
