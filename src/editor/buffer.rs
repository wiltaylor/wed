use crate::app::BufferId;
use crate::editor::{History, Marks, Registers};
use ropey::Rope;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Buffer {
    pub id: BufferId,
    pub rope: Rope,
    pub path: Option<PathBuf>,
    pub language_id: Option<String>,
    pub dirty: bool,
    pub history: History,
    pub registers: Registers,
    pub marks: Marks,
    pub diagnostics: Vec<lsp_types::Diagnostic>,
    pub version: i32,
}
