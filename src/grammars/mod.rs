//! Vendored tree-sitter grammar bindings for grammars not available as
//! compatible crates on crates.io.

pub mod gitattributes {
    use tree_sitter_language::LanguageFn;

    extern "C" {
        fn tree_sitter_gitattributes() -> *const ();
    }

    pub const LANGUAGE: LanguageFn =
        unsafe { LanguageFn::from_raw(tree_sitter_gitattributes) };
}

pub mod gitignore {
    use tree_sitter_language::LanguageFn;

    extern "C" {
        fn tree_sitter_gitignore() -> *const ();
    }

    pub const LANGUAGE: LanguageFn =
        unsafe { LanguageFn::from_raw(tree_sitter_gitignore) };
}
