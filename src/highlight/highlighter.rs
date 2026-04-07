use ropey::Rope;
use std::collections::HashMap;
use tree_sitter::{InputEdit, Parser, Query, QueryCursor, Tree};

use crate::app::BufferId;
use crate::config::Theme;
use crate::editor::Buffer;
use crate::highlight::grammar_registry::{GrammarEntry, GrammarRegistry};
use crate::highlight::theme_map::capture_to_style;

/// A highlight span: byte range + ratatui style.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub capture: String,
    pub style: ratatui::style::Style,
}

/// Per-buffer parsing state.
pub struct Highlighter {
    parser: Parser,
    tree: Option<Tree>,
    language_id: String,
}

impl Highlighter {
    pub fn new(entry: &GrammarEntry) -> anyhow::Result<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&entry.language)
            .map_err(|e| anyhow::anyhow!("failed to set language: {e}"))?;
        Ok(Self {
            parser,
            tree: None,
            language_id: entry.id.to_string(),
        })
    }

    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    pub fn parse_full(&mut self, text: &Rope) {
        let tree = parse_rope(&mut self.parser, text, None);
        self.tree = tree;
    }

    pub fn parse_incremental(&mut self, text: &Rope, edit: InputEdit) {
        if let Some(tree) = self.tree.as_mut() {
            tree.edit(&edit);
        }
        let old = self.tree.clone();
        self.tree = parse_rope(&mut self.parser, text, old.as_ref());
    }
}

fn parse_rope(parser: &mut Parser, text: &Rope, old: Option<&Tree>) -> Option<Tree> {
    parser.parse_with(
        &mut |byte_offset: usize, _position| -> &[u8] {
            if byte_offset >= text.len_bytes() {
                return &[];
            }
            let (chunk, chunk_byte_idx, _, _) = text.chunk_at_byte(byte_offset);
            let local = byte_offset - chunk_byte_idx;
            &chunk.as_bytes()[local..]
        },
        old,
    )
}

/// Run a tree-sitter query against `tree` and return raw spans.
pub fn highlight_spans(tree: &Tree, query: &Query, source: &[u8]) -> Vec<(usize, usize, String)> {
    let mut cursor = QueryCursor::new();
    let capture_names = query.capture_names();
    let mut out = Vec::new();
    let matches = cursor.matches(query, tree.root_node(), source);
    for m in matches {
        for cap in m.captures {
            let name = capture_names[cap.index as usize].to_string();
            let node = cap.node;
            out.push((node.start_byte(), node.end_byte(), name));
        }
    }
    out
}

/// Engine that owns one Highlighter per buffer and produces themed spans.
#[derive(Default)]
pub struct HighlightEngine {
    highlighters: HashMap<BufferId, Highlighter>,
    queries: HashMap<String, Query>,
}

impl HighlightEngine {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_highlighter(&mut self, buffer: &Buffer) -> Option<&mut Highlighter> {
        let registry = GrammarRegistry::global();
        let entry = if let Some(lang) = buffer.language_id.as_deref() {
            registry.for_language(lang)
        } else if let Some(p) = buffer.path.as_ref() {
            registry.for_path(p)
        } else {
            None
        }?;

        if !self.highlighters.contains_key(&buffer.id) {
            let h = Highlighter::new(entry).ok()?;
            self.highlighters.insert(buffer.id, h);
        }
        if !self.queries.contains_key(entry.id) {
            let q = Query::new(&entry.language, entry.highlights_query).ok()?;
            self.queries.insert(entry.id.to_string(), q);
        }
        self.highlighters.get_mut(&buffer.id)
    }

    pub fn highlight(&mut self, buffer: &Buffer) -> Vec<HighlightSpan> {
        self.highlight_with_theme(buffer, &Theme::tokyo_night())
    }

    pub fn highlight_with_theme(&mut self, buffer: &Buffer, theme: &Theme) -> Vec<HighlightSpan> {
        let lang_id = match self.ensure_highlighter(buffer) {
            Some(h) => h.language_id().to_string(),
            None => return Vec::new(),
        };

        // Re-parse fully (callers can use parse_incremental directly for edits).
        let highlighter = self.highlighters.get_mut(&buffer.id).unwrap();
        highlighter.parse_full(&buffer.rope);
        let tree = match highlighter.tree() {
            Some(t) => t.clone(),
            None => return Vec::new(),
        };

        let query = match self.queries.get(&lang_id) {
            Some(q) => q,
            None => return Vec::new(),
        };

        let source = buffer.rope.to_string();
        let raw = highlight_spans(&tree, query, source.as_bytes());
        raw.into_iter()
            .map(|(s, e, name)| {
                let style = capture_to_style(&name, theme);
                HighlightSpan {
                    start_byte: s,
                    end_byte: e,
                    capture: name,
                    style,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_snippet() {
        let src = "fn main() { let x = 1; }";
        let mut buffer = Buffer {
            id: BufferId(1),
            rope: Rope::from_str(src),
            path: Some(std::path::PathBuf::from("snippet.rs")),
            language_id: Some("rust".into()),
            ..Default::default()
        };
        let _ = &mut buffer;

        let mut engine = HighlightEngine::new();
        let spans = engine.highlight(&buffer);
        assert!(!spans.is_empty(), "expected non-empty highlight spans");

        // Find a `keyword` span covering "fn" at byte 0..2.
        let kw = spans
            .iter()
            .find(|s| s.capture == "keyword" && s.start_byte == 0 && s.end_byte == 2);
        assert!(
            kw.is_some(),
            "expected keyword 'fn' at 0..2; got {:?}",
            spans
        );

        // Find a `function` span covering "main" at byte 3..7.
        let func = spans
            .iter()
            .find(|s| s.capture == "function" && s.start_byte == 3 && s.end_byte == 7);
        assert!(
            func.is_some(),
            "expected function 'main' at 3..7; got {:?}",
            spans
        );
    }
}
