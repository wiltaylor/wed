use crate::layout::Pane;

/// A bottom panel that mirrors `Sidebar` but stretches horizontally and
/// reserves vertical space at the bottom of the editor area.
pub struct BottomPanel {
    pub open: bool,
    pub height: u16,
    pub panes: Vec<Box<dyn Pane>>,
    pub active: usize,
}

impl Default for BottomPanel {
    fn default() -> Self {
        Self {
            open: false,
            height: 12,
            panes: Vec::new(),
            active: 0,
        }
    }
}

impl std::fmt::Debug for BottomPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BottomPanel")
            .field("open", &self.open)
            .field("height", &self.height)
            .field("panes", &self.panes.len())
            .field("active", &self.active)
            .finish()
    }
}
