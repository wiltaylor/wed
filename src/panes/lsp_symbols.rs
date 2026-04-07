use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct LspSymbolsPane;

#[async_trait]
impl Pane for LspSymbolsPane {
    fn name(&self) -> &str { "lsp_symbols" }
}
