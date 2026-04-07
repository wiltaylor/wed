pub mod breakpoints;
pub mod client;
pub mod protocol;
pub mod session;
pub mod ui;

pub use client::DapClient;

use std::collections::HashMap;

use crate::app::SessionId;

#[derive(Default)]
pub struct DapManager {
    pub sessions: HashMap<SessionId, DapClient>,
}

impl DapManager {
    pub fn new() -> Self {
        Self::default()
    }
}
