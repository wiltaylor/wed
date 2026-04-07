use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub name: String,
    pub source: Option<String>,
    pub line: usize,
}

#[derive(Default)]
pub struct DapCallStackPane {
    pub frames: Vec<StackFrame>,
    pub selected: usize,
}

impl DapCallStackPane {
    pub fn new() -> Self { Self::default() }
    pub fn set_frames(&mut self, frames: Vec<StackFrame>) {
        self.frames = frames;
        self.selected = 0;
    }
}

/// Backwards compatible alias for the lower-cased file name.
pub type DapCallstackPane = DapCallStackPane;

#[async_trait]
impl Pane for DapCallStackPane {
    fn name(&self) -> &str { "dap_callstack" }
}
