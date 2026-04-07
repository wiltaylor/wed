use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct DapBreakpointsPane;

#[async_trait]
impl Pane for DapBreakpointsPane {
    fn name(&self) -> &str { "dap_breakpoints" }
}
