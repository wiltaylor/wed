pub mod capabilities;
pub mod client;
pub mod code_actions;
pub mod completion;
pub mod diagnostics;
pub mod hover;
pub mod protocol;
pub mod rename;
pub mod signature_help;

pub use client::LspClient;

use std::collections::HashMap;

use crate::app::ServerId;

#[derive(Default)]
pub struct LspManager {
    pub clients: HashMap<ServerId, LspClient>,
}

impl LspManager {
    pub fn new() -> Self {
        Self::default()
    }
}
