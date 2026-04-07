use crate::layout::View;

#[derive(Debug)]
pub enum SplitNode {
    Leaf(View),
    Horizontal(Vec<SplitNode>),
    Vertical(Vec<SplitNode>),
}

impl Default for SplitNode {
    fn default() -> Self {
        SplitNode::Leaf(View::default())
    }
}
