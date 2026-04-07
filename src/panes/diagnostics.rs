use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct DiagnosticsPane;

#[async_trait]
impl Pane for DiagnosticsPane {
    fn name(&self) -> &str { "diagnostics" }
}
