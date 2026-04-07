use crate::layout::SplitNode;

#[derive(Debug, Default)]
pub struct Tab {
    pub root: SplitNode,
    pub name: String,
}
