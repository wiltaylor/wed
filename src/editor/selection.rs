use crate::editor::Cursor;

#[derive(Debug, Clone, Copy, Default)]
pub struct Selection {
    pub anchor: Cursor,
    pub head: Cursor,
}
