use once_cell::sync::Lazy;
use std::path::Path;
use tree_sitter::Language;

/// Minimal highlights query for Rust.
const RUST_HIGHLIGHTS: &str = r#"
[
  "fn" "let" "const" "static" "if" "else" "match" "for" "while"
  "loop" "return" "break" "continue" "struct" "enum" "trait" "impl"
  "pub" "use" "mod" "as" "in" "where" "move"
] @keyword

(string_literal) @string
(raw_string_literal) @string
(char_literal) @string

(line_comment) @comment
(block_comment) @comment

(function_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))

(type_identifier) @type
(primitive_type) @type
"#;

/// Minimal highlights query for JSON.
const JSON_HIGHLIGHTS: &str = r#"
[
  "true" "false" "null"
] @keyword

(string) @string
(number) @type
(comment) @comment
"#;

pub struct GrammarEntry {
    pub id: &'static str,
    pub language: Language,
    pub highlights_query: &'static str,
}

pub struct GrammarRegistry {
    entries: Vec<GrammarEntry>,
}

impl GrammarRegistry {
    fn build() -> Self {
        Self {
            entries: vec![
                GrammarEntry {
                    id: "rust",
                    language: tree_sitter_rust::language(),
                    highlights_query: RUST_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "json",
                    language: tree_sitter_json::language(),
                    highlights_query: JSON_HIGHLIGHTS,
                },
            ],
        }
    }

    pub fn global() -> &'static GrammarRegistry {
        static REGISTRY: Lazy<GrammarRegistry> = Lazy::new(GrammarRegistry::build);
        &REGISTRY
    }

    pub fn for_language(&self, id: &str) -> Option<&GrammarEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn for_path(&self, path: &Path) -> Option<&GrammarEntry> {
        let ext = path.extension()?.to_str()?;
        let id = match ext {
            "rs" => "rust",
            "json" => "json",
            _ => return None,
        };
        self.for_language(id)
    }
}

impl Default for GrammarRegistry {
    fn default() -> Self {
        Self::build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_by_extension() {
        let reg = GrammarRegistry::global();
        assert_eq!(reg.for_path(Path::new("foo.rs")).map(|e| e.id), Some("rust"));
        assert_eq!(
            reg.for_path(Path::new("foo.json")).map(|e| e.id),
            Some("json")
        );
        assert!(reg.for_path(Path::new("foo.txt")).is_none());
    }

    #[test]
    fn lookup_by_id() {
        let reg = GrammarRegistry::global();
        assert!(reg.for_language("rust").is_some());
        assert!(reg.for_language("nope").is_none());
    }
}
