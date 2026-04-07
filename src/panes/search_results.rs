use crate::layout::Pane;
use async_trait::async_trait;

#[derive(Default)]
pub struct SearchResultsPane;

#[async_trait]
impl Pane for SearchResultsPane {
    fn name(&self) -> &str { "search_results" }
}
