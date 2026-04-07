//! UI state for the hover popup.

use lsp_types::HoverContents;

#[derive(Debug, Clone)]
pub struct HoverPopup {
    pub contents: HoverContents,
}

impl HoverPopup {
    pub fn new(contents: HoverContents) -> Self {
        Self { contents }
    }
}
