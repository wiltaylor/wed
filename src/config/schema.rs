//! TOML configuration schema for wed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub line_numbers: bool,
    pub relative_line_numbers: bool,
    pub tab_width: u32,
    pub expand_tabs: bool,
    pub scroll_off: u32,
    pub cursor_style: String,
    pub auto_indent: bool,
    pub smart_indent: bool,
    pub wrap: bool,
    pub highlight_line: bool,
    pub color_column: Option<u32>,
    pub show_whitespace: bool,
    pub undo_limit: usize,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            line_numbers: true,
            relative_line_numbers: false,
            tab_width: 4,
            expand_tabs: true,
            scroll_off: 4,
            cursor_style: "block".into(),
            auto_indent: true,
            smart_indent: true,
            wrap: false,
            highlight_line: true,
            color_column: None,
            show_whitespace: false,
            undo_limit: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub tabline: bool,
    pub statusline: bool,
    pub left_sidebar_width: u16,
    pub right_sidebar_width: u16,
    pub popup_width: u16,
    pub popup_height: u16,
    pub icons: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            tabline: true,
            statusline: true,
            left_sidebar_width: 30,
            right_sidebar_width: 40,
            popup_width: 60,
            popup_height: 15,
            icons: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub case_sensitive: bool,
    pub hidden_files: bool,
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            hidden_files: false,
            max_results: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct TerminalConfig {
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspConfig {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub filetypes: Vec<String>,
    #[serde(default)]
    pub root_patterns: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DapConfig {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub port_range: Option<(u16, u16)>,
    #[serde(default)]
    pub configurations: Vec<toml::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FiletypeConfig {
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub language_id: String,
    #[serde(default)]
    pub tab_width: Option<u32>,
    #[serde(default)]
    pub expand_tabs: Option<bool>,
    #[serde(default)]
    pub comment: Option<String>,
}
