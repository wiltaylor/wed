use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub bg: String,
    pub bg_dark: String,
    pub bg_highlight: String,
    pub fg: String,
    pub fg_dark: String,
    pub fg_gutter: String,
    pub comment: String,
    pub blue: String,
    pub cyan: String,
    pub green: String,
    pub magenta: String,
    pub orange: String,
    pub purple: String,
    pub red: String,
    pub yellow: String,
}

impl Default for Theme {
    fn default() -> Self {
        // Tokyo Night
        Self {
            bg: "#1a1b26".into(),
            bg_dark: "#16161e".into(),
            bg_highlight: "#292e42".into(),
            fg: "#c0caf5".into(),
            fg_dark: "#a9b1d6".into(),
            fg_gutter: "#3b4261".into(),
            comment: "#565f89".into(),
            blue: "#7aa2f7".into(),
            cyan: "#7dcfff".into(),
            green: "#9ece6a".into(),
            magenta: "#bb9af7".into(),
            orange: "#ff9e64".into(),
            purple: "#9d7cd8".into(),
            red: "#f7768e".into(),
            yellow: "#e0af68".into(),
        }
    }
}
