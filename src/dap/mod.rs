//! Debug Adapter Protocol subsystem.
//!
//! Layout:
//! - `protocol`: wire framing + typed message enum
//! - `client`: transport (stdio/tcp), seq-correlated request/response
//! - `session`: high-level commands and debuggee state
//! - `breakpoints`: persistent breakpoint store
//! - `ui`: pure state for debug UI overlays
//!
//! `DapManager` owns all live sessions and the shared breakpoint store.

pub mod breakpoints;
pub mod client;
pub mod protocol;
pub mod session;
pub mod ui;

pub use breakpoints::{Breakpoint, BreakpointStore};
pub use client::{DapClient, DapResponse};
pub use protocol::{read_message, write_message, DapMessage};
pub use session::{DapSession, DapThread, Scope, StackFrame, Variable};
pub use ui::DebugOverlayState;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::app::{AppEvent, SessionId};

/// Owns all DAP sessions and the persistent breakpoint store. Held by `App`.
pub struct DapManager {
    pub sessions: HashMap<SessionId, DapSession>,
    pub breakpoints: BreakpointStore,
    pub active_session: Option<SessionId>,
    pub event_tx: Option<mpsc::UnboundedSender<AppEvent>>,
    next_id: u64,
}

impl Default for DapManager {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            breakpoints: BreakpointStore::default(),
            active_session: None,
            event_tx: None,
            next_id: 1,
        }
    }
}

impl DapManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach an event sender. Must be called before starting sessions.
    pub fn set_event_tx(&mut self, tx: mpsc::UnboundedSender<AppEvent>) {
        self.event_tx = Some(tx);
    }

    /// Load the breakpoint store from `<root>/.wed/breakpoints.json`.
    pub fn load_breakpoints(&mut self, root: &Path) -> Result<()> {
        self.breakpoints = BreakpointStore::load(root)?;
        Ok(())
    }

    fn alloc_id(&mut self) -> SessionId {
        let id = SessionId(self.next_id);
        self.next_id += 1;
        id
    }

    fn require_tx(&self) -> Result<mpsc::UnboundedSender<AppEvent>> {
        self.event_tx
            .clone()
            .ok_or_else(|| anyhow::anyhow!("DapManager event_tx not set"))
    }

    /// Spawn a debug adapter as a child process and register the session.
    pub async fn start_stdio(
        &mut self,
        name: String,
        program: &str,
        args: &[String],
    ) -> Result<SessionId> {
        let tx = self.require_tx()?;
        let id = self.alloc_id();
        let client = DapClient::spawn_stdio(id, name, program, args, tx).await?;
        let session = DapSession::new(Arc::new(client));
        self.sessions.insert(id, session);
        self.active_session = Some(id);
        Ok(id)
    }

    /// Connect to a TCP-hosted debug adapter and register the session.
    pub async fn start_tcp(&mut self, name: String, host: &str, port: u16) -> Result<SessionId> {
        let tx = self.require_tx()?;
        let id = self.alloc_id();
        let client = DapClient::connect_tcp(id, name, host, port, tx).await?;
        let session = DapSession::new(Arc::new(client));
        self.sessions.insert(id, session);
        self.active_session = Some(id);
        Ok(id)
    }

    /// Terminate and drop a session.
    pub async fn stop(&mut self, id: SessionId) -> Result<()> {
        if let Some(s) = self.sessions.get(&id) {
            let _ = s.terminate().await;
        }
        self.sessions.remove(&id);
        if self.active_session == Some(id) {
            self.active_session = self.sessions.keys().next().copied();
        }
        Ok(())
    }

    pub fn session(&self, id: SessionId) -> Option<&DapSession> {
        self.sessions.get(&id)
    }

    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut DapSession> {
        self.sessions.get_mut(&id)
    }

    pub fn active(&self) -> Option<&DapSession> {
        self.active_session.and_then(|id| self.sessions.get(&id))
    }

    /// Convenience: forward an arbitrary event into the channel.
    pub fn forward(&self, event: AppEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Convenience: launch config on the active session.
    pub async fn launch_active(&self, config: Value) -> Result<()> {
        if let Some(s) = self.active() {
            s.launch(config).await?;
        }
        Ok(())
    }
}
