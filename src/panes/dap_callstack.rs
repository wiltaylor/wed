use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct DapCallstackPane;

#[async_trait]
impl Pane for DapCallstackPane {
    fn name(&self) -> &str { "dap_callstack" }
}
