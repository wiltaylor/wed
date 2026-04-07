use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EditorConfig {
    pub tab_width: Option<u32>,
    pub expand_tab: Option<bool>,
    pub line_numbers: Option<bool>,
    pub relative_line_numbers: Option<bool>,
    pub scrolloff: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiConfig {
    pub show_tabline: Option<bool>,
    pub show_statusline: Option<bool>,
    pub sidebar_width: Option<u16>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspConfig {
    pub command: String,
    pub args: Vec<String>,
    pub filetypes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DapConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FiletypeConfig {
    pub extensions: Vec<String>,
    pub language_id: String,
}
