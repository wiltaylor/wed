use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Parse a color from a hex string like `#rrggbb` or `#rgb`.
pub fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&s[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&s[2..3], 16).ok()? * 17;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

fn hex(s: &str) -> Color {
    parse_hex(s).unwrap_or(Color::Reset)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxColors {
    pub keyword: String,
    pub string: String,
    pub comment: String,
    pub function: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub variable: String,
    pub constant: String,
    pub number: String,
    pub operator: String,
    pub punctuation: String,
    pub property: String,
    pub parameter: String,
    pub tag: String,
    pub attribute: String,
}

impl SyntaxColors {
    fn tokyo_night() -> Self {
        Self {
            keyword: "#bb9af7".into(),
            string: "#9ece6a".into(),
            comment: "#565f89".into(),
            function: "#7aa2f7".into(),
            type_: "#2ac3de".into(),
            variable: "#c0caf5".into(),
            constant: "#ff9e64".into(),
            number: "#ff9e64".into(),
            operator: "#89ddff".into(),
            punctuation: "#a9b1d6".into(),
            property: "#73daca".into(),
            parameter: "#e0af68".into(),
            tag: "#f7768e".into(),
            attribute: "#bb9af7".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeColors {
    pub normal: String,
    pub insert: String,
    pub visual: String,
    pub command: String,
    pub replace: String,
    pub pending: String,
    pub debug: String,
}

impl ModeColors {
    fn tokyo_night() -> Self {
        Self {
            normal: "#7aa2f7".into(),
            insert: "#9ece6a".into(),
            visual: "#bb9af7".into(),
            command: "#e0af68".into(),
            replace: "#f7768e".into(),
            pending: "#ff9e64".into(),
            debug: "#f7768e".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitColors {
    pub added: String,
    pub modified: String,
    pub deleted: String,
    pub renamed: String,
    pub untracked: String,
    pub conflict: String,
}

impl GitColors {
    fn tokyo_night() -> Self {
        Self {
            added: "#9ece6a".into(),
            modified: "#e0af68".into(),
            deleted: "#f7768e".into(),
            renamed: "#7aa2f7".into(),
            untracked: "#bb9af7".into(),
            conflict: "#ff9e64".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticColors {
    pub error: String,
    pub warning: String,
    pub info: String,
    pub hint: String,
}

impl DiagnosticColors {
    fn tokyo_night() -> Self {
        Self {
            error: "#f7768e".into(),
            warning: "#e0af68".into(),
            info: "#7dcfff".into(),
            hint: "#1abc9c".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapColors {
    pub breakpoint: String,
    pub current_line: String,
    pub stopped: String,
    pub log_point: String,
}

impl DapColors {
    fn tokyo_night() -> Self {
        Self {
            breakpoint: "#f7768e".into(),
            current_line: "#e0af68".into(),
            stopped: "#bb9af7".into(),
            log_point: "#7dcfff".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    pub gutter: String,
    pub gutter_active: String,
    pub status_bar_bg: String,
    pub status_bar_fg: String,
    pub line_highlight: String,
    pub syntax: SyntaxColors,
    pub modes: ModeColors,
    pub git: GitColors,
    pub diagnostics: DiagnosticColors,
    pub dap: DapColors,
}

impl Default for Theme {
    fn default() -> Self {
        Self::tokyo_night()
    }
}

impl Theme {
    pub fn tokyo_night() -> Self {
        Self {
            background: "#1a1b26".into(),
            foreground: "#c0caf5".into(),
            cursor: "#c0caf5".into(),
            selection: "#283457".into(),
            gutter: "#3b4261".into(),
            gutter_active: "#737aa2".into(),
            status_bar_bg: "#16161e".into(),
            status_bar_fg: "#a9b1d6".into(),
            line_highlight: "#292e42".into(),
            syntax: SyntaxColors::tokyo_night(),
            modes: ModeColors::tokyo_night(),
            git: GitColors::tokyo_night(),
            diagnostics: DiagnosticColors::tokyo_night(),
            dap: DapColors::tokyo_night(),
        }
    }

    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    // Convenience accessors returning ratatui Colors.
    pub fn background_color(&self) -> Color {
        hex(&self.background)
    }
    pub fn foreground_color(&self) -> Color {
        hex(&self.foreground)
    }
    pub fn cursor_color(&self) -> Color {
        hex(&self.cursor)
    }
    pub fn selection_color(&self) -> Color {
        hex(&self.selection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_6() {
        assert_eq!(parse_hex("#1a1b26"), Some(Color::Rgb(0x1a, 0x1b, 0x26)));
        assert_eq!(parse_hex("ffffff"), Some(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn parse_hex_3() {
        assert_eq!(parse_hex("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(parse_hex("#000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn parse_hex_invalid() {
        assert_eq!(parse_hex("notahex"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
    }

    #[test]
    fn theme_toml_roundtrip() {
        let theme = Theme::tokyo_night();
        let s = theme.to_toml_string().unwrap();
        let parsed = Theme::from_toml_str(&s).unwrap();
        assert_eq!(parsed.background, theme.background);
        assert_eq!(parsed.syntax.keyword, theme.syntax.keyword);
        assert_eq!(parsed.modes.insert, theme.modes.insert);
        assert_eq!(parsed.git.added, theme.git.added);
        assert_eq!(parsed.diagnostics.error, theme.diagnostics.error);
        assert_eq!(parsed.dap.breakpoint, theme.dap.breakpoint);
    }
}
