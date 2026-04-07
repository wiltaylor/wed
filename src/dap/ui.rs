//! Pure state for DAP UI overlays. Rendering lives elsewhere (Agent B);
//! this module only owns the data the renderer reads.

use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct DebugOverlayState {
    /// File and 1-based line where execution is currently stopped.
    pub current_line: Option<(PathBuf, u32)>,
    /// Inline variable hints to render at end-of-line: (line, name, value).
    pub inline_vars: Vec<(u32, String, String)>,
}

impl DebugOverlayState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.current_line = None;
        self.inline_vars.clear();
    }

    pub fn set_current_line(&mut self, file: PathBuf, line: u32) {
        self.current_line = Some((file, line));
    }

    pub fn push_inline_var(&mut self, line: u32, name: impl Into<String>, value: impl Into<String>) {
        self.inline_vars.push((line, name.into(), value.into()));
    }
}
