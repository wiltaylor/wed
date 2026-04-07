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
    pub pending: crate::input::pending::PendingState,
    pub last_change: crate::commands::context::LastChange,
    pub search: crate::editor::search::SearchState,
    pub want_col: usize,
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
            pending: crate::input::pending::PendingState::default(),
            last_change: crate::commands::context::LastChange::default(),
            search: crate::editor::search::SearchState::default(),
            want_col: 0,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        use crossterm::event::{
            DisableMouseCapture, EnableMouseCapture, Event as CtEvent, EventStream,
        };
        use crossterm::execute;
        use crossterm::terminal::{
            disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
        };
        use futures::StreamExt;
        use ratatui::backend::CrosstermBackend;
        use ratatui::Terminal;

        // Ctrl+C → Quit
        let tx_sig = self.event_tx.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx_sig.send(AppEvent::Quit);
        });

        // Spawn crossterm event reader.
        let tx_ev = self.event_tx.clone();
        let input_task = tokio::spawn(async move {
            let mut stream = EventStream::new();
            while let Some(Ok(ev)) = stream.next().await {
                let app_ev = match ev {
                    CtEvent::Key(k) => AppEvent::Key(k),
                    CtEvent::Mouse(m) => AppEvent::Mouse(m),
                    CtEvent::Resize(w, h) => AppEvent::Resize(w, h),
                    CtEvent::Paste(s) => AppEvent::Paste(s),
                    _ => continue,
                };
                if tx_ev.send(app_ev).is_err() {
                    break;
                }
            }
        });

        // Terminal setup. Use a guard so panics still restore the terminal.
        struct TermGuard;
        impl Drop for TermGuard {
            fn drop(&mut self) {
                let _ = disable_raw_mode();
                let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
            }
        }
        enable_raw_mode().ok();
        execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture).ok();
        let _guard = TermGuard;

        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;

        // Initial draw.
        terminal.draw(|f| crate::render::render(f, self))?;

        while !self.should_quit {
            let mut dirty = false;
            // Block on the first event, then drain any others non-blockingly.
            let first = self.event_rx.recv().await;
            match first {
                Some(AppEvent::Quit) => {
                    self.should_quit = true;
                    dirty = true;
                }
                Some(_ev) => {
                    dirty = true;
                }
                None => break,
            }
            while let Ok(ev) = self.event_rx.try_recv() {
                if matches!(ev, AppEvent::Quit) {
                    self.should_quit = true;
                }
                dirty = true;
            }

            if dirty && !self.should_quit {
                terminal.draw(|f| crate::render::render(f, self))?;
            }
        }

        input_task.abort();
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
