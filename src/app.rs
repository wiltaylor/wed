use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::commands::CommandRegistry;
use crate::config::Config;
use crate::dap::DapManager;
use crate::editor::Buffer;
use crate::input::EditorMode;
use crate::layout::LayoutState;
use crate::lsp::LspManager;

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
        pub struct $name(pub u64);
    };
}

id_newtype!(BufferId);
id_newtype!(ViewId);
id_newtype!(RequestId);
id_newtype!(ServerId);
id_newtype!(SessionId);

#[derive(Debug)]
pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),
    Paste(String),
    LspDiagnostics { server: ServerId, uri: String },
    LspCompletion { request: RequestId },
    LspHover { request: RequestId },
    LspDefinition { request: RequestId },
    LspReferences { request: RequestId },
    LspSignature { request: RequestId },
    LspCodeActions { request: RequestId },
    LspRename { request: RequestId },
    LspServerExit { server: ServerId },
    DapStopped { session: SessionId },
    DapContinued { session: SessionId },
    DapOutput { session: SessionId, text: String },
    DapTerminated { session: SessionId },
    DapBreakpointVerified { session: SessionId },
    FileChanged(std::path::PathBuf),
    ConfigReloaded,
    GitStatusUpdated,
    Render,
    Quit,
}

pub struct App {
    pub config: Config,
    pub mode: EditorMode,
    pub buffers: Vec<Buffer>,
    pub layout: LayoutState,
    pub commands: CommandRegistry,
    pub lsp: LspManager,
    pub dap: DapManager,
    pub event_tx: mpsc::UnboundedSender<AppEvent>,
    pub event_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            config: Config::default(),
            mode: EditorMode::Normal,
            buffers: Vec::new(),
            layout: LayoutState::default(),
            commands: CommandRegistry::new(),
            lsp: LspManager::new(),
            dap: DapManager::new(),
            event_tx,
            event_rx,
            should_quit: false,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Minimal: register a Ctrl+C handler that triggers Quit, then drain events.
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(AppEvent::Quit);
        });

        while !self.should_quit {
            match self.event_rx.recv().await {
                Some(AppEvent::Quit) => self.should_quit = true,
                Some(_) => {}
                None => break,
            }
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
