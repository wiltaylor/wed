use crate::app::{BufferId, ViewId};
use crate::editor::Selection;

/// A view into a buffer. `cursor` is kept as a simple `(row, col)` tuple to avoid
/// coupling with `editor::Cursor` which is owned by Agent A.
#[derive(Debug, Default, Clone)]
pub struct View {
    pub id: ViewId,
    pub buffer_id: BufferId,
    pub cursor: (usize, usize),
    pub scroll: (usize, usize),
    pub selection: Option<Selection>,
}

impl View {
    pub fn new(id: ViewId, buffer_id: BufferId) -> Self {
        Self {
            id,
            buffer_id,
            cursor: (0, 0),
            scroll: (0, 0),
            selection: None,
        }
    }

    /// Translate a screen position inside the view's text area into a (row, col)
    /// buffer position, accounting for scroll and gutter width.
    pub fn screen_to_buffer(&self, screen_row: u16, screen_col: u16, gutter_w: u16) -> (usize, usize) {
        let col = screen_col.saturating_sub(gutter_w) as usize + self.scroll.1;
        let row = screen_row as usize + self.scroll.0;
        (row, col)
    }
}
