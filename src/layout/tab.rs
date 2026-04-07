use crate::app::ViewId;
use crate::layout::SplitNode;
use crate::panes::git_history::GitHistoryPane;

/// What lives inside a tab. Most tabs are buffer-backed (split tree of
/// editor views); special tabs (e.g. file history) own a pane directly.
pub enum TabKind {
    Buffer,
    GitHistory(GitHistoryPane),
}

impl Default for TabKind {
    fn default() -> Self {
        TabKind::Buffer
    }
}

impl std::fmt::Debug for TabKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabKind::Buffer => f.write_str("Buffer"),
            TabKind::GitHistory(_) => f.write_str("GitHistory"),
        }
    }
}

#[derive(Debug, Default)]
pub struct Tab {
    pub name: String,
    pub root: SplitNode,
    pub active_view: ViewId,
    pub kind: TabKind,
}

impl Tab {
    pub fn new(name: impl Into<String>, root: SplitNode, active_view: ViewId) -> Self {
        Self {
            name: name.into(),
            root,
            active_view,
            kind: TabKind::Buffer,
        }
    }

    pub fn new_git_history(name: impl Into<String>, pane: GitHistoryPane) -> Self {
        Self {
            name: name.into(),
            root: SplitNode::default(),
            active_view: ViewId(0),
            kind: TabKind::GitHistory(pane),
        }
    }
}
