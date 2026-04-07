pub mod pane;
pub mod sidebar;
pub mod split;
pub mod tab;
pub mod view;

pub use pane::Pane;
pub use sidebar::Sidebar;
pub use split::SplitNode;
pub use tab::Tab;
pub use view::View;

#[derive(Debug, Default)]
pub struct LayoutState {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub left_sidebar: Sidebar,
    pub right_sidebar: Sidebar,
}
