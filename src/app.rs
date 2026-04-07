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
    LspHoverContents { contents: Option<lsp_types::Hover> },
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
    pub command_line: crate::commands::command_line::CommandLineState,
    pub status_message: Option<(String, bool)>,
    pub keybindings: crate::config::keybindings::Keybindings,
    pub leader_seq: Option<Vec<crate::input::keys::Key>>,
    pub leader_popup_visible: bool,
    pub picker: Option<crate::panes::picker::Picker<std::path::PathBuf>>,
    pub picker_query: String,
    pub sidebar_focused: bool,
    pub last_editor_rect: ratatui::layout::Rect,
    pub last_editor_view_rects: Vec<(ViewId, ratatui::layout::Rect)>,
    pub last_left_sidebar_rect: ratatui::layout::Rect,
    pub last_tab_rects: Vec<ratatui::layout::Rect>,
    pub last_tab_close_rects: Vec<ratatui::layout::Rect>,
    pub last_sidebar_click_row: Option<usize>,
    pub want_col: usize,
    pub highlight: crate::highlight::HighlightEngine,
    /// Buffer indices waiting for an LSP `start_server` + `did_open`.
    pub pending_lsp_attach: Vec<usize>,
    /// Active LSP info popup (diagnostics + hover) shown on `<leader>k`.
    pub hover_popup: Option<HoverPopup>,
}

/// State for the on-demand info popup triggered by `lsp.hover`.
#[derive(Debug, Clone, Default)]
pub struct HoverPopup {
    /// Cursor row when the popup was opened (used to anchor + auto-dismiss).
    pub anchor_row: usize,
    pub anchor_col: usize,
    /// Diagnostics overlapping the cursor at trigger time.
    pub diagnostics: Vec<lsp_types::Diagnostic>,
    /// `Some(text)` once the hover response arrives; `None` while waiting.
    pub hover_text: Option<String>,
    /// True until the hover response (or absence) is received.
    pub loading: bool,
}

impl App {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let mut commands = CommandRegistry::new();
        crate::commands::definitions::register_all(&mut commands);
        Self {
            config: Config::default(),
            mode: EditorMode::Normal,
            buffers: Vec::new(),
            layout: LayoutState::default(),
            commands,
            lsp: LspManager::with_event_tx(event_tx.clone()),
            dap: DapManager::new(),
            event_tx,
            event_rx,
            should_quit: false,
            pending: crate::input::pending::PendingState::default(),
            last_change: crate::commands::context::LastChange::default(),
            search: crate::editor::search::SearchState::default(),
            command_line: crate::commands::command_line::CommandLineState::new(),
            status_message: None,
            keybindings: crate::config::keybindings::Keybindings::defaults(),
            leader_seq: None,
            leader_popup_visible: false,
            picker: None,
            picker_query: String::new(),
            sidebar_focused: false,
            last_editor_rect: ratatui::layout::Rect::default(),
            last_editor_view_rects: Vec::new(),
            last_left_sidebar_rect: ratatui::layout::Rect::default(),
            last_tab_rects: Vec::new(),
            last_tab_close_rects: Vec::new(),
            last_sidebar_click_row: None,
            want_col: 0,
            highlight: crate::highlight::HighlightEngine::new(),
            pending_lsp_attach: Vec::new(),
            hover_popup: None,
        }
    }

    /// Drain `pending_lsp_attach` and start servers + send `did_open`.
    /// Called from the async run loop after each event dispatch.
    async fn drain_pending_lsp_attach(&mut self) {
        let pending = std::mem::take(&mut self.pending_lsp_attach);
        tracing::info!(
            "drain_pending_lsp_attach: {} pending, lsp config keys: {:?}",
            pending.len(),
            self.config.lsp.keys().collect::<Vec<_>>()
        );
        for idx in pending {
            let Some(buf) = self.buffers.get(idx) else {
                continue;
            };
            let Some(lang_id) = buf.language_id.clone() else {
                tracing::info!("buffer {idx} has no language_id");
                continue;
            };
            tracing::info!("buffer {idx} language_id={lang_id}");
            let Some(cfg) = self.config.lsp.get(&lang_id).cloned() else {
                tracing::info!("no lsp config for language {lang_id}");
                continue;
            };
            tracing::info!("starting lsp server for {lang_id}: {}", cfg.command);
            self.lsp.starting.insert(lang_id.clone());
            let Some(path) = buf.path.clone() else {
                continue;
            };
            let abs_path = std::fs::canonicalize(&path).unwrap_or(path);
            let root = resolve_workspace_root(&abs_path, &cfg.root_patterns);
            let start_result = self
                .lsp
                .start_server(lang_id.clone(), &cfg.command, &cfg.args, root)
                .await;
            self.lsp.starting.remove(&lang_id);
            if let Err(e) = start_result {
                tracing::warn!("lsp start_server({lang_id}) failed: {e:#}");
                self.status_message = Some((format!("lsp {lang_id}: {e}"), true));
                continue;
            }
            let Some(uri) = path_to_uri(&abs_path) else {
                continue;
            };
            let text;
            let version;
            {
                let buf = &mut self.buffers[idx];
                buf.lsp_uri = Some(uri.clone());
                buf.version = 1;
                buf.lsp_dirty = false;
                version = buf.version;
                text = buf.rope.to_string();
            }
            if let Err(e) = self.lsp.did_open(uri, lang_id, version, text).await {
                tracing::warn!("lsp did_open failed: {e:#}");
            }
        }
    }

    /// Open a file in a new tab. If the file is already open in an
    /// existing tab, switch to that tab instead.
    pub fn open_file_in_new_tab(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        // Reuse an existing tab if this file is already open.
        for (i, b) in self.buffers.iter().enumerate() {
            if b.path.as_deref() == Some(path) {
                // Find the tab whose active view points at this buffer.
                for (ti, tab) in self.layout.tabs.iter().enumerate() {
                    if let Some(v) = tab.root.find(tab.active_view) {
                        if v.buffer_id.0 as usize == i {
                            self.layout.active_tab = ti;
                            return Ok(());
                        }
                    }
                }
            }
        }
        let mut buf = crate::editor::Buffer::from_path(path)?;
        let new_idx = self.buffers.len();
        buf.id = BufferId(new_idx as u64);
        // Defer LSP start/did_open to the async run loop.
        if buf.language_id.is_some() {
            self.pending_lsp_attach.push(new_idx);
        }
        self.buffers.push(buf);
        let vid = ViewId(new_idx as u64 + 1);
        let mut view = crate::layout::View::new(vid, BufferId(new_idx as u64));
        view.cursor = (0, 0);
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let tab = crate::layout::Tab::new(name, crate::layout::SplitNode::Leaf(view), vid);
        self.layout.tabs.push(tab);
        self.layout.active_tab = self.layout.tabs.len() - 1;
        Ok(())
    }

    /// Dispatch a single AppEvent into editor state.
    pub fn dispatch_event(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Quit => self.should_quit = true,
            AppEvent::LspHoverContents { contents } => {
                if let Some(p) = &mut self.hover_popup {
                    p.loading = false;
                    p.hover_text = contents.and_then(hover_to_text);
                }
                return;
            }
            AppEvent::Key(k) => {
                // Any key dismisses the hover popup; the `lsp.hover` command
                // (triggered via leader) repopulates it after KeyHandler runs.
                self.hover_popup = None;
                let key = crate::input::keys::Key::from_event(k);
                crate::input::key_handler::KeyHandler::handle(self, key);
                // Only push edits to the LSP once we're out of insert mode —
                // this lets the user finish a line without rust-analyzer
                // cancelling its in-flight diagnostic computation on every
                // keystroke. Normal-mode edits (x, dd, p, undo…) still flush
                // immediately since this branch runs after KeyHandler.
                if !matches!(self.mode, crate::input::EditorMode::Insert) {
                    self.flush_lsp_did_change();
                }
            }
            AppEvent::Mouse(m) => {
                crate::input::key_handler::KeyHandler::mouse(self, m);
            }
            AppEvent::Resize(_, _) => {}
            AppEvent::Paste(s) => {
                crate::input::key_handler::KeyHandler::paste(self, &s);
                self.flush_lsp_did_change();
            }
            _ => {}
        }
    }

    /// Open the on-demand info popup at the cursor: collects diagnostics
    /// overlapping the cursor (snapped to words, matching the underline)
    /// and fires an async `textDocument/hover` request whose result lands
    /// back via `AppEvent::LspHoverContents`.
    pub fn trigger_hover_popup(&mut self) {
        let Some(tab) = self.layout.active_tab() else {
            return;
        };
        let Some(view) = tab.root.find(tab.active_view) else {
            return;
        };
        let cursor_row = view.cursor.0;
        let cursor_col = view.cursor.1;
        let buffer_idx = view.buffer_id.0 as usize;
        let Some(buf) = self.buffers.get(buffer_idx) else {
            return;
        };
        let Some(uri) = buf.lsp_uri.clone() else {
            self.status_message = Some(("no LSP attached".into(), false));
            return;
        };
        let lang_id = match &buf.language_id {
            Some(l) => l.clone(),
            None => return,
        };

        // Snapshot diagnostics whose (word-snapped) range covers the cursor.
        let store = self.lsp.diagnostics.lock();
        let diags_all = store.get(&uri).to_vec();
        drop(store);
        let diagnostics: Vec<lsp_types::Diagnostic> = diags_all
            .into_iter()
            .filter(|d| diagnostic_covers_cursor(buf, d, cursor_row, cursor_col))
            .collect();

        self.hover_popup = Some(HoverPopup {
            anchor_row: cursor_row,
            anchor_col: cursor_col,
            diagnostics,
            hover_text: None,
            loading: true,
        });

        // Fire hover request in the background.
        let Some(client) = self.lsp.client_for_language(&lang_id) else {
            // No live client; mark popup as not-loading so it shows just diags.
            if let Some(p) = &mut self.hover_popup {
                p.loading = false;
            }
            return;
        };
        let event_tx = self.event_tx.clone();
        let position = lsp_types::Position {
            line: cursor_row as u32,
            character: cursor_col as u32,
        };
        tokio::spawn(async move {
            use lsp_types::request::Request as LspRequest;
            use lsp_types::{HoverParams, TextDocumentIdentifier, TextDocumentPositionParams};
            let params = HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position,
                },
                work_done_progress_params: Default::default(),
            };
            let res: Result<Option<lsp_types::Hover>, _> = client
                .request(
                    <lsp_types::request::HoverRequest as LspRequest>::METHOD,
                    params,
                )
                .await;
            let contents = match res {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("lsp hover request failed: {e:#}");
                    None
                }
            };
            let _ = event_tx.send(AppEvent::LspHoverContents { contents });
        });
    }

    /// If the active buffer has pending edits and an attached LSP URI,
    /// send a full-document `textDocument/didChange` in the background.
    fn flush_lsp_did_change(&mut self) {
        let Some(tab) = self.layout.active_tab() else {
            return;
        };
        let Some(view) = tab.root.find(tab.active_view) else {
            return;
        };
        let idx = view.buffer_id.0 as usize;
        let Some(buf) = self.buffers.get_mut(idx) else {
            return;
        };
        if !buf.lsp_dirty {
            return;
        }
        let Some(uri) = buf.lsp_uri.clone() else {
            buf.lsp_dirty = false;
            return;
        };
        let Some(lang_id) = buf.language_id.clone() else {
            return;
        };
        let Some(client) = self.lsp.client_for_language(&lang_id) else {
            return;
        };
        let version = buf.version;
        let text = buf.rope.to_string();
        let text_len = text.len();
        buf.lsp_dirty = false;
        tracing::info!("lsp queue_did_change v={version} bytes={text_len}");
        // Push through the client's debouncer — rapid keystrokes coalesce
        // into a single `didChange` so rust-analyzer isn't constantly
        // cancelling in-flight diagnostic computations.
        client.queue_did_change(crate::lsp::client::DidChangeRequest {
            uri,
            version,
            text,
        });
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

        fn apply_cursor_style(mode: EditorMode) {
            use crossterm::cursor::SetCursorStyle;
            let style = match mode {
                EditorMode::Insert => SetCursorStyle::BlinkingBar,
                EditorMode::Replace => SetCursorStyle::BlinkingUnderScore,
                _ => SetCursorStyle::BlinkingBlock,
            };
            let _ = execute!(std::io::stdout(), style);
        }

        // Pre-mark languages as starting so the first draw shows "[LSP …]"
        // while the (blocking) initialize handshake runs.
        for &idx in &self.pending_lsp_attach {
            if let Some(buf) = self.buffers.get(idx) {
                if let Some(lid) = &buf.language_id {
                    if self.config.lsp.contains_key(lid) {
                        self.lsp.starting.insert(lid.clone());
                    }
                }
            }
        }

        // Initial draw — do this BEFORE attaching LSPs so the user sees
        // the file immediately, even if `initialize` takes a moment.
        apply_cursor_style(self.mode);
        {
            let app_ref: &mut App = self;
            terminal.draw(|f| crate::render::render(f, app_ref))?;
        }

        // Attach LSPs for any files opened before the run loop started.
        self.drain_pending_lsp_attach().await;
        {
            let app_ref: &mut App = self;
            terminal.draw(|f| crate::render::render(f, app_ref))?;
        }

        while !self.should_quit {
            // Block on the first event, with a 200ms debounce race when a
            // leader sequence is pending but the which-key popup isn't yet
            // shown — the sleep branch flips the popup visible on timeout.
            let debounce_active = self.leader_seq.is_some() && !self.leader_popup_visible;
            let first_opt = if debounce_active {
                tokio::select! {
                    ev = self.event_rx.recv() => Some(ev),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => None,
                }
            } else {
                Some(self.event_rx.recv().await)
            };
            match first_opt {
                Some(Some(ev)) => self.dispatch_event(ev),
                Some(None) => break,
                None => self.leader_popup_visible = true,
            }
            while let Ok(ev) = self.event_rx.try_recv() {
                self.dispatch_event(ev);
            }

            if !self.pending_lsp_attach.is_empty() {
                self.drain_pending_lsp_attach().await;
            }

            if !self.should_quit {
                apply_cursor_style(self.mode);
                let app_ref: &mut App = self;
                terminal.draw(|f| crate::render::render(f, app_ref))?;
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

/// Walk up from `file` looking for any ancestor containing one of
/// `patterns`. Falls back to the file's parent directory, and finally
/// the current working directory.
fn resolve_workspace_root(file: &std::path::Path, patterns: &[String]) -> std::path::PathBuf {
    let default_patterns: &[&str] = &["Cargo.toml", ".git"];
    let owned: Vec<&str> = if patterns.is_empty() {
        default_patterns.to_vec()
    } else {
        patterns.iter().map(|s| s.as_str()).collect()
    };
    let start = file.parent().unwrap_or(std::path::Path::new("."));
    for ancestor in start.ancestors() {
        for p in &owned {
            if ancestor.join(p).exists() {
                return ancestor.to_path_buf();
            }
        }
    }
    start.to_path_buf()
}

/// Mirrors the underline rendering's word-snapping so the popup hit area
/// matches what the user actually sees underlined.
pub(crate) fn diagnostic_covers_cursor(
    buf: &crate::editor::Buffer,
    d: &lsp_types::Diagnostic,
    cursor_row: usize,
    cursor_col: usize,
) -> bool {
    let s_line = d.range.start.line as usize;
    let e_line = d.range.end.line as usize;
    let s_col = d.range.start.character as usize;
    let e_col = d.range.end.character as usize;

    if cursor_row < s_line || cursor_row > e_line {
        return false;
    }
    if cursor_row > s_line && cursor_row < e_line {
        return true;
    }

    let line_chars: Vec<char> = buf.rope.line(s_line).chars().collect();
    let line_len = line_chars.iter().take_while(|c| **c != '\n').count();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';

    let (snap_s, snap_e) = if e_col <= s_col || s_col >= line_len {
        if line_len == 0 {
            (0usize, 0usize)
        } else {
            let probe = s_col.min(line_len.saturating_sub(1));
            let mut ws = probe;
            while ws > 0 && !is_word(line_chars[ws]) {
                ws -= 1;
            }
            if is_word(line_chars[ws]) {
                while ws > 0 && is_word(line_chars[ws - 1]) {
                    ws -= 1;
                }
                let mut we = ws;
                while we < line_len && is_word(line_chars[we]) {
                    we += 1;
                }
                (ws, we)
            } else {
                (probe, probe + 1)
            }
        }
    } else {
        (s_col, e_col.min(line_len))
    };

    if cursor_row == s_line && cursor_row == e_line {
        cursor_col >= snap_s && cursor_col < snap_e.max(snap_s + 1)
    } else if cursor_row == s_line {
        cursor_col >= snap_s
    } else {
        cursor_col < e_col.max(1)
    }
}

/// Flatten an LSP `Hover` response into a single plain-text string.
pub(crate) fn hover_to_text(hover: lsp_types::Hover) -> Option<String> {
    use lsp_types::{HoverContents, MarkedString};
    let s = match hover.contents {
        HoverContents::Scalar(MarkedString::String(s)) => s,
        HoverContents::Scalar(MarkedString::LanguageString(ls)) => ls.value,
        HoverContents::Array(items) => items
            .into_iter()
            .map(|m| match m {
                MarkedString::String(s) => s,
                MarkedString::LanguageString(ls) => ls.value,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        HoverContents::Markup(m) => m.value,
    };
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Convert a filesystem path into an `lsp_types::Uri` (`file://…`).
pub(crate) fn path_to_uri(path: &std::path::Path) -> Option<lsp_types::Uri> {
    use std::str::FromStr;
    let url = url::Url::from_file_path(path).ok()?;
    lsp_types::Uri::from_str(url.as_str()).ok()
}
