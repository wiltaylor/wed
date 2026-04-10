fn main() {
    // Compile vendored tree-sitter grammars that aren't available as
    // compatible crates on crates.io.

    cc::Build::new()
        .include("grammars/tree-sitter-gitattributes/src")
        .file("grammars/tree-sitter-gitattributes/src/parser.c")
        .compile("tree-sitter-gitattributes");

    cc::Build::new()
        .include("grammars/tree-sitter-gitignore/src")
        .file("grammars/tree-sitter-gitignore/src/parser.c")
        .compile("tree-sitter-gitignore");
}
