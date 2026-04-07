use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct DapVariablesPane;

#[async_trait]
impl Pane for DapVariablesPane {
    fn name(&self) -> &str { "dap_variables" }
}
