//! UI state for the signature help popup.

use lsp_types::SignatureHelp;

#[derive(Debug, Clone)]
pub struct SignatureHelpPopup {
    pub help: SignatureHelp,
    pub active_signature: usize,
}

impl SignatureHelpPopup {
    pub fn new(help: SignatureHelp) -> Self {
        let active_signature = help.active_signature.unwrap_or(0) as usize;
        Self {
            help,
            active_signature,
        }
    }

    pub fn next(&mut self) {
        let n = self.help.signatures.len();
        if n == 0 {
            return;
        }
        self.active_signature = (self.active_signature + 1) % n;
    }

    pub fn prev(&mut self) {
        let n = self.help.signatures.len();
        if n == 0 {
            return;
        }
        self.active_signature = if self.active_signature == 0 {
            n - 1
        } else {
            self.active_signature - 1
        };
    }
}
