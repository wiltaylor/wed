use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct FileBrowserPane;

#[async_trait]
impl Pane for FileBrowserPane {
    fn name(&self) -> &str { "file_browser" }
}
