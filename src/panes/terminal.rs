use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct TerminalPane;

#[async_trait]
impl Pane for TerminalPane {
    fn name(&self) -> &str { "terminal" }
}
