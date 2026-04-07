pub mod bottom_panel;
pub mod pane;
pub mod sidebar;
pub mod split;
pub mod tab;
pub mod view;

pub use bottom_panel::BottomPanel;
pub use pane::Pane;
pub use sidebar::Sidebar;
pub use split::{Direction, SplitNode};
pub use tab::Tab;
pub use view::View;

#[derive(Debug, Default)]
pub struct LayoutState {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub left_sidebar: Sidebar,
    pub right_sidebar: Sidebar,
    pub bottom_panel: BottomPanel,
}

impl LayoutState {
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }
}
