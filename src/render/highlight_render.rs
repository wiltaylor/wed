use ratatui::style::Style;

/// Map a language token name to a style. Stub until highlight module wires real styles.
pub fn style_for_token(_token: &str) -> Style {
    // TODO: integrate with crate::highlight once theme/highlight module is ready.
    Style::default()
}
