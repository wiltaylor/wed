use crate::layout::Pane;

pub struct Sidebar {
    pub open: bool,
    pub width: u16,
    pub panes: Vec<Box<dyn Pane>>,
    pub active: usize,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            open: false,
            width: 30,
            panes: Vec::new(),
            active: 0,
        }
    }
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
