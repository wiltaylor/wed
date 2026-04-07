pub mod grammar_registry;
pub mod highlighter;
pub mod theme_map;

pub use grammar_registry::{GrammarEntry, GrammarRegistry};
pub use highlighter::{HighlightEngine, HighlightSpan, Highlighter, highlight_spans};
pub use theme_map::capture_to_style;
