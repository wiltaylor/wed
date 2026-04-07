use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct GitPane;

#[async_trait]
impl Pane for GitPane {
    fn name(&self) -> &str { "git" }
}
