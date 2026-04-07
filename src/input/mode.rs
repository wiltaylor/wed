#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualKind {
    Char,
    Line,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Visual(VisualKind),
    Replace,
    Command,
    Search,
    Pending,
    Operator,
}

impl Default for EditorMode {
    fn default() -> Self { EditorMode::Normal }
}
