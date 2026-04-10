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

const MARKDOWN_HIGHLIGHTS: &str = r#"
(atx_heading) @keyword
(setext_heading) @keyword
(fenced_code_block) @string
(indented_code_block) @string
(block_quote) @comment
(thematic_break) @punctuation
(list_marker_minus) @punctuation
(list_marker_plus) @punctuation
(list_marker_star) @punctuation
(list_marker_dot) @punctuation
"#;

const TOML_HIGHLIGHTS: &str = r#"
(comment) @comment
(string) @string
(integer) @number
(float) @number
(boolean) @keyword
(bare_key) @property
(table ["[" "]"] @punctuation)
(table_array_element ["[[" "]]"] @punctuation)
"#;

const PYTHON_HIGHLIGHTS: &str = r#"
[
  "def" "return" "if" "elif" "else" "for" "while" "in" "not" "and" "or"
  "class" "import" "from" "as" "pass" "break" "continue" "lambda"
  "try" "except" "finally" "raise" "with" "yield" "global" "nonlocal"
  "assert" "del" "is"
] @keyword

[ "True" "False" "None" ] @constant

(string) @string
(integer) @number
(float) @number
(comment) @comment

(function_definition name: (identifier) @function)
(call function: (identifier) @function)
(call function: (attribute attribute: (identifier) @function))

(class_definition name: (identifier) @type)
"#;

const JAVASCRIPT_HIGHLIGHTS: &str = r#"
[
  "function" "return" "if" "else" "for" "while" "do" "switch" "case"
  "default" "break" "continue" "var" "let" "const" "new" "delete"
  "typeof" "instanceof" "in" "of" "class" "extends" "import" "export"
  "from" "as" "try" "catch" "finally" "throw" "async" "await" "yield"
  "this"
] @keyword

[ "true" "false" "null" "undefined" ] @constant

(string) @string
(template_string) @string
(regex) @string
(number) @number
(comment) @comment

(function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (member_expression property: (property_identifier) @function))
"#;

const BASH_HIGHLIGHTS: &str = r#"
[
  "if" "then" "else" "elif" "fi" "case" "esac" "for" "while" "do" "done"
  "in" "function" "return" "local" "declare" "export"
] @keyword

(comment) @comment
(string) @string
(raw_string) @string
(variable_name) @variable
(command_name) @function
"#;

const HTML_HIGHLIGHTS: &str = r#"
(tag_name) @tag
(attribute_name) @attribute
(attribute_value) @string
(comment) @comment
(doctype) @keyword
"#;

const CSS_HIGHLIGHTS: &str = r#"
(comment) @comment
(string_value) @string
(integer_value) @number
(float_value) @number
(tag_name) @tag
(class_name) @type
(id_name) @type
(property_name) @property
(plain_value) @variable
"#;

const YAML_HIGHLIGHTS: &str = r#"
(comment) @comment
(string_scalar) @string
(integer_scalar) @number
(float_scalar) @number
(boolean_scalar) @keyword
(null_scalar) @constant
(block_mapping_pair key: (flow_node) @property)
"#;

const JUST_HIGHLIGHTS: &str = r#"
["export" "import"] @keyword

"mod" @keyword

["alias" "set" "shell"] @keyword

["if" "else"] @keyword

(value (identifier) @variable)
(alias left: (identifier) @variable)
(assignment left: (identifier) @variable)

(recipe_header name: (identifier) @function)
(dependency name: (identifier) @function)
(dependency_expression name: (identifier) @function)
(function_call name: (identifier) @function)

(parameter name: (identifier) @variable.parameter)

(module name: (identifier) @type)

[
  ":=" "?" "==" "!=" "=~" "@" "=" "$" "*" "+" "&&" "@-" "-@" "-" "/" ":"
] @operator

"," @punctuation.delimiter

["{" "}" "[" "]" "(" ")" "{{" "}}"] @punctuation.bracket

["`" "```"] @punctuation.special

(boolean) @keyword

[(string) (external_command)] @string

(escape_sequence) @string.escape

(comment) @comment

(shebang) @keyword

(setting
  left: (identifier) @keyword
  (#any-of? @keyword
    "allow-duplicate-recipes"
    "allow-duplicate-variables"
    "dotenv-filename"
    "dotenv-load"
    "dotenv-path"
    "dotenv-required"
    "export"
    "fallback"
    "ignore-comments"
    "positional-arguments"
    "shell"
    "shell-interpreter"
    "tempdir"
    "windows-powershell"
    "windows-shell"
    "working-directory"))

(attribute
  (identifier) @attribute
  (#any-of? @attribute
    "confirm"
    "doc"
    "extension"
    "group"
    "linux"
    "macos"
    "metadata"
    "no-cd"
    "no-exit-message"
    "no-quiet"
    "openbsd"
    "parallel"
    "positional-arguments"
    "private"
    "script"
    "unix"
    "windows"
    "working-directory"))

(numeric_error) @keyword
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
                    language: tree_sitter_rust::LANGUAGE.into(),
                    highlights_query: RUST_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "json",
                    language: tree_sitter_json::LANGUAGE.into(),
                    highlights_query: JSON_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "markdown",
                    language: tree_sitter_md::LANGUAGE.into(),
                    highlights_query: MARKDOWN_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "toml",
                    language: tree_sitter_toml_ng::LANGUAGE.into(),
                    highlights_query: TOML_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "python",
                    language: tree_sitter_python::LANGUAGE.into(),
                    highlights_query: PYTHON_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "javascript",
                    language: tree_sitter_javascript::LANGUAGE.into(),
                    highlights_query: JAVASCRIPT_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "bash",
                    language: tree_sitter_bash::LANGUAGE.into(),
                    highlights_query: BASH_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "html",
                    language: tree_sitter_html::LANGUAGE.into(),
                    highlights_query: HTML_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "css",
                    language: tree_sitter_css::LANGUAGE.into(),
                    highlights_query: CSS_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "yaml",
                    language: tree_sitter_yaml::LANGUAGE.into(),
                    highlights_query: YAML_HIGHLIGHTS,
                },
                GrammarEntry {
                    id: "just",
                    language: tree_sitter_just::LANGUAGE.into(),
                    highlights_query: JUST_HIGHLIGHTS,
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
        // Filename-based lookups for files without a real extension.
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            match name {
                ".bashrc" | ".bash_profile" | "bashrc" => return self.for_language("bash"),
                "justfile" | "Justfile" | ".justfile" => return self.for_language("just"),
                _ => {}
            }
        }
        let ext = path.extension()?.to_str()?;
        let id = match ext {
            "rs" => "rust",
            "json" | "jsonc" => "json",
            "md" | "markdown" => "markdown",
            "toml" => "toml",
            "py" | "pyi" => "python",
            "js" | "mjs" | "cjs" | "jsx" => "javascript",
            "sh" | "bash" => "bash",
            "html" | "htm" => "html",
            "css" => "css",
            "yaml" | "yml" => "yaml",
            "just" => "just",
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
        assert_eq!(
            reg.for_path(Path::new("foo.rs")).map(|e| e.id),
            Some("rust")
        );
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
