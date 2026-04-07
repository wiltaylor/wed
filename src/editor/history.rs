#[derive(Debug, Default)]
pub struct History {
    pub undo: Vec<()>,
    pub redo: Vec<()>,
}
