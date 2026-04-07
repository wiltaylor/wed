use crate::app::ViewId;
use crate::layout::SplitNode;

#[derive(Debug, Default)]
pub struct Tab {
    pub name: String,
    pub root: SplitNode,
    pub active_view: ViewId,
}

impl Tab {
    pub fn new(name: impl Into<String>, root: SplitNode, active_view: ViewId) -> Self {
        Self {
            name: name.into(),
            root,
            active_view,
        }
    }
}
