pub mod keybindings;
pub mod schema;
pub mod theme;

pub use keybindings::KeybindingConfig;
pub use schema::{DapConfig, EditorConfig, FiletypeConfig, LspConfig, UiConfig};
pub use theme::Theme;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub editor: EditorConfig,
    pub ui: UiConfig,
    pub theme: Theme,
    pub keybindings: KeybindingConfig,
    pub lsp: HashMap<String, LspConfig>,
    pub dap: HashMap<String, DapConfig>,
    pub filetypes: HashMap<String, FiletypeConfig>,
}
