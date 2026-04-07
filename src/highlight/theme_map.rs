use ratatui::style::{Color, Style};

use crate::config::Theme;
use crate::config::theme::parse_hex;

fn color(s: &str) -> Color {
    parse_hex(s).unwrap_or(Color::Reset)
}

/// Map a tree-sitter capture name to a ratatui [`Style`] using the theme.
///
/// Capture names follow the dotted convention used by tree-sitter
/// highlights queries (e.g. `function.method`, `string.special`); we match
/// on the leading segment.
pub fn capture_to_style(capture_name: &str, theme: &Theme) -> Style {
    let head = capture_name.split('.').next().unwrap_or(capture_name);
    let fg = match head {
        "keyword" => &theme.syntax.keyword,
        "string" => &theme.syntax.string,
        "comment" => &theme.syntax.comment,
        "function" => &theme.syntax.function,
        "type" => &theme.syntax.type_,
        "variable" => &theme.syntax.variable,
        "constant" => &theme.syntax.constant,
        "number" => &theme.syntax.number,
        "operator" => &theme.syntax.operator,
        "punctuation" => &theme.syntax.punctuation,
        "property" => &theme.syntax.property,
        "parameter" => &theme.syntax.parameter,
        "tag" => &theme.syntax.tag,
        "attribute" => &theme.syntax.attribute,
        _ => &theme.foreground,
    };
    Style::default().fg(color(fg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_captures() {
        let theme = Theme::tokyo_night();
        let s = capture_to_style("keyword", &theme);
        assert_eq!(s.fg, parse_hex(&theme.syntax.keyword));
        let s = capture_to_style("function.method", &theme);
        assert_eq!(s.fg, parse_hex(&theme.syntax.function));
    }

    #[test]
    fn unknown_capture_falls_back_to_foreground() {
        let theme = Theme::tokyo_night();
        let s = capture_to_style("does_not_exist", &theme);
        assert_eq!(s.fg, parse_hex(&theme.foreground));
    }
}
