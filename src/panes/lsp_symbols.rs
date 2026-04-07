use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct SymbolNode {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub children: Vec<SymbolNode>,
}

#[derive(Default)]
pub struct LspSymbolsPane {
    pub tree: Vec<SymbolNode>,
    pub selected: usize,
}

impl LspSymbolsPane {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_tree(&mut self, tree: Vec<SymbolNode>) {
        self.tree = tree;
        self.selected = 0;
    }
}

#[async_trait]
impl Pane for LspSymbolsPane {
    fn name(&self) -> &str {
        "lsp_symbols"
    }
}
