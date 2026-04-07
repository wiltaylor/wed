use crate::layout::Pane;

#[derive(Default)]
pub struct Sidebar {
    pub open: bool,
    pub width: u16,
    pub panes: Vec<Box<dyn Pane>>,
    pub active: usize,
}

impl std::fmt::Debug for Sidebar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sidebar")
            .field("open", &self.open)
            .field("width", &self.width)
            .field("panes", &self.panes.len())
            .field("active", &self.active)
            .finish()
    }
}
