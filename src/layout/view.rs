use crate::app::{BufferId, ViewId};
use crate::editor::{Cursor, Selection};

#[derive(Debug, Default)]
pub struct View {
    pub id: ViewId,
    pub buffer_id: BufferId,
    pub cursor: Cursor,
    pub scroll: (usize, usize),
    pub selection: Option<Selection>,
}
