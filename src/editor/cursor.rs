#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub want_col: usize,
}
