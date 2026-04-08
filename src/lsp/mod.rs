//! LSP subsystem: spawns language servers and proxies requests/notifications.

pub mod apply_edit;
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
pub use diagnostics::DiagnosticStore;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use lsp_types::{
    request::Request as LspRequest, ClientInfo, CodeActionContext, CodeActionParams,
    CodeActionResponse, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentFormattingParams, DocumentSymbolParams, DocumentSymbolResponse, FormattingOptions,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, InitializeParams,
    InitializeResult, InitializedParams, InlayHint, InlayHintParams, Location, Position,
    PublishDiagnosticsParams, Range, ReferenceContext, ReferenceParams, RenameParams,
    SemanticTokensParams, SemanticTokensResult, SignatureHelp, SignatureHelpParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, TextEdit, Uri, VersionedTextDocumentIdentifier, WorkspaceEdit,
    WorkspaceFolder,
};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::str::FromStr;
use tokio::sync::mpsc;

use crate::app::{AppEvent, RequestId, ServerId};

/// Identifies a server by (workspace root, language id).
pub type ServerKey = (PathBuf, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    None,
    Starting,
    Ready,
}

pub struct LspManager {
    pub clients: HashMap<ServerKey, Arc<LspClient>>,
    pub diagnostics: Arc<Mutex<DiagnosticStore>>,
    pub event_tx: Option<mpsc::UnboundedSender<AppEvent>>,
    /// Language ids whose `initialize` handshake is currently in flight.
    pub starting: HashSet<String>,
    next_server_id: u64,
    next_request_id: u64,
}

impl Default for LspManager {
    fn default() -> Self {
        Self {
            clients: HashMap::new(),
            diagnostics: Arc::new(Mutex::new(DiagnosticStore::new())),
            event_tx: None,
            starting: HashSet::new(),
            next_server_id: 1,
            next_request_id: 1,
        }
    }
}

impl LspManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_event_tx(event_tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        Self {
            event_tx: Some(event_tx),
            ..Self::default()
        }
    }

    /// Status of an LSP server for a given language id.
    pub fn server_status(&self, language_id: &str) -> ServerStatus {
        if self.client_for_language(language_id).is_some() {
            ServerStatus::Ready
        } else if self.starting.contains(language_id) {
            ServerStatus::Starting
        } else {
            ServerStatus::None
        }
    }

    fn alloc_request_id(&mut self) -> RequestId {
        let id = self.next_request_id;
        self.next_request_id += 1;
        RequestId(id)
    }

    fn alloc_server_id(&mut self) -> ServerId {
        let id = self.next_server_id;
        self.next_server_id += 1;
        ServerId(id)
    }

    fn post(&self, ev: AppEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(ev);
        }
    }

    /// Spawn a server, perform the `initialize`/`initialized` handshake, and
    /// register it under `(root, language_id)`.
    pub async fn start_server(
        &mut self,
        language_id: impl Into<String>,
        command: &str,
        args: &[String],
        root: PathBuf,
    ) -> Result<Arc<LspClient>> {
        let language_id = language_id.into();
        let key: ServerKey = (root.clone(), language_id.clone());
        if let Some(c) = self.clients.get(&key) {
            return Ok(c.clone());
        }
        let server_id = self.alloc_server_id();
        let client = LspClient::spawn(
            server_id,
            language_id.clone(),
            command,
            args,
            self.event_tx.clone(),
        )
        .await?;

        // Pump notifications to the diagnostic store / event_tx.
        if let Some(rx) = client.notifications.lock().take() {
            let diags = self.diagnostics.clone();
            let event_tx = self.event_tx.clone();
            tokio::spawn(async move {
                let mut rx = rx;
                while let Some((method, params)) = rx.recv().await {
                    if method == "textDocument/publishDiagnostics" {
                        if let Ok(p) =
                            serde_json::from_value::<PublishDiagnosticsParams>(params.clone())
                        {
                            let uri_str = p.uri.to_string();
                            tracing::info!(
                                "lsp[{}] publishDiagnostics {}: {} items",
                                server_id.0,
                                uri_str,
                                p.diagnostics.len()
                            );
                            for d in &p.diagnostics {
                                tracing::info!(
                                    "  diag [{}:{}..{}:{}] sev={:?} msg={}",
                                    d.range.start.line,
                                    d.range.start.character,
                                    d.range.end.line,
                                    d.range.end.character,
                                    d.severity,
                                    d.message.lines().next().unwrap_or("")
                                );
                            }
                            diags.lock().publish(p);
                            if let Some(tx) = &event_tx {
                                let _ = tx.send(AppEvent::LspDiagnostics {
                                    server: server_id,
                                    uri: uri_str,
                                });
                            }
                        }
                    }
                }
                if let Some(tx) = &event_tx {
                    let _ = tx.send(AppEvent::LspServerExit { server: server_id });
                }
            });
        }

        let root_uri = url::Url::from_file_path(&root)
            .ok()
            .and_then(|u| Uri::from_str(u.as_str()).ok());
        // Encourage rust-analyzer to publish parser/HIR diagnostics
        // promptly on every `didChange` instead of only after `cargo check`
        // (which by default runs on save).
        let init_options = if language_id == "rust" {
            Some(json!({
                "diagnostics": {
                    "experimental": { "enable": true },
                    "refreshSupport": true,
                },
                "checkOnSave": false,
            }))
        } else {
            None
        };

        #[allow(deprecated)]
        let init = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: root_uri.clone(),
            initialization_options: init_options,
            capabilities: capabilities::client_capabilities(),
            trace: None,
            workspace_folders: root_uri.as_ref().map(|u| {
                vec![WorkspaceFolder {
                    uri: u.clone(),
                    name: root
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                }]
            }),
            client_info: Some(ClientInfo {
                name: "wed".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            locale: None,
            ..Default::default()
        };
        tracing::info!("lsp[{}]: sending initialize", server_id.0);
        let _: InitializeResult = client.request("initialize", init).await?;
        tracing::info!("lsp[{}]: initialize ok, sending initialized", server_id.0);
        client.notify("initialized", InitializedParams {}).await?;
        tracing::info!("lsp[{}]: initialized sent", server_id.0);

        self.clients.insert(key, client.clone());
        Ok(client)
    }

    fn any_client(&self) -> Option<&Arc<LspClient>> {
        self.clients.values().next()
    }

    /// Return the first running client whose language id matches.
    pub fn client_for_language(&self, language_id: &str) -> Option<Arc<LspClient>> {
        self.clients
            .iter()
            .find(|((_, lang), _)| lang == language_id)
            .map(|(_, c)| c.clone())
    }

    // ---- Text document sync notifications ----

    pub async fn did_open(
        &self,
        uri: Uri,
        language_id: String,
        version: i32,
        text: String,
    ) -> Result<()> {
        let Some(client) = self.any_client() else {
            return Ok(());
        };
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id,
                version,
                text,
            },
        };
        client.notify("textDocument/didOpen", params).await
    }

    pub async fn did_change(
        &self,
        uri: Uri,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Result<()> {
        let Some(client) = self.any_client() else {
            return Ok(());
        };
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: changes,
        };
        client.notify("textDocument/didChange", params).await
    }

    pub async fn did_save(&self, uri: Uri, text: Option<String>) -> Result<()> {
        let Some(client) = self.any_client() else {
            return Ok(());
        };
        let params = DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text,
        };
        client.notify("textDocument/didSave", params).await
    }

    pub async fn did_close(&self, uri: Uri) -> Result<()> {
        let Some(client) = self.any_client() else {
            return Ok(());
        };
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        };
        client.notify("textDocument/didClose", params).await
    }

    // ---- Requests ----

    fn text_doc_pos(uri: Uri, pos: Position) -> TextDocumentPositionParams {
        TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: pos,
        }
    }

    pub async fn completion(
        &mut self,
        uri: Uri,
        pos: Position,
    ) -> Result<Option<CompletionResponse>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = CompletionParams {
            text_document_position: Self::text_doc_pos(uri, pos),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };
        let r: Option<CompletionResponse> = client
            .request(
                <lsp_types::request::Completion as LspRequest>::METHOD,
                params,
            )
            .await?;
        self.post(AppEvent::LspCompletion { request: req_id });
        Ok(r)
    }

    pub async fn hover(&mut self, uri: Uri, pos: Position) -> Result<Option<Hover>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = HoverParams {
            text_document_position_params: Self::text_doc_pos(uri, pos),
            work_done_progress_params: Default::default(),
        };
        let r: Option<Hover> = client
            .request(
                <lsp_types::request::HoverRequest as LspRequest>::METHOD,
                params,
            )
            .await?;
        self.post(AppEvent::LspHover { request: req_id });
        Ok(r)
    }

    pub async fn signature_help(
        &mut self,
        uri: Uri,
        pos: Position,
    ) -> Result<Option<SignatureHelp>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = SignatureHelpParams {
            text_document_position_params: Self::text_doc_pos(uri, pos),
            work_done_progress_params: Default::default(),
            context: None,
        };
        let r: Option<SignatureHelp> = client
            .request(
                <lsp_types::request::SignatureHelpRequest as LspRequest>::METHOD,
                params,
            )
            .await?;
        self.post(AppEvent::LspSignature { request: req_id });
        Ok(r)
    }

    pub async fn definition(
        &mut self,
        uri: Uri,
        pos: Position,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = GotoDefinitionParams {
            text_document_position_params: Self::text_doc_pos(uri, pos),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let r: Option<GotoDefinitionResponse> = client
            .request(
                <lsp_types::request::GotoDefinition as LspRequest>::METHOD,
                params,
            )
            .await?;
        self.post(AppEvent::LspDefinition { request: req_id });
        Ok(r)
    }

    pub async fn implementation(
        &mut self,
        uri: Uri,
        pos: Position,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = GotoDefinitionParams {
            text_document_position_params: Self::text_doc_pos(uri, pos),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let r: Option<GotoDefinitionResponse> = client
            .request(
                <lsp_types::request::GotoImplementation as LspRequest>::METHOD,
                params,
            )
            .await?;
        self.post(AppEvent::LspDefinition { request: req_id });
        Ok(r)
    }

    pub async fn references(&mut self, uri: Uri, pos: Position) -> Result<Option<Vec<Location>>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = ReferenceParams {
            text_document_position: Self::text_doc_pos(uri, pos),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: ReferenceContext {
                include_declaration: true,
            },
        };
        let r: Option<Vec<Location>> = client
            .request(
                <lsp_types::request::References as LspRequest>::METHOD,
                params,
            )
            .await?;
        self.post(AppEvent::LspReferences { request: req_id });
        Ok(r)
    }

    pub async fn code_action(
        &mut self,
        uri: Uri,
        range: Range,
    ) -> Result<Option<CodeActionResponse>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range,
            context: CodeActionContext {
                diagnostics: self.diagnostics.lock().get(&uri).to_vec(),
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let r: Option<CodeActionResponse> = client
            .request(
                <lsp_types::request::CodeActionRequest as LspRequest>::METHOD,
                params,
            )
            .await?;
        self.post(AppEvent::LspCodeActions { request: req_id });
        Ok(r)
    }

    pub async fn rename(
        &mut self,
        uri: Uri,
        pos: Position,
        new_name: String,
    ) -> Result<Option<WorkspaceEdit>> {
        let req_id = self.alloc_request_id();
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = RenameParams {
            text_document_position: Self::text_doc_pos(uri, pos),
            new_name,
            work_done_progress_params: Default::default(),
        };
        let r: Option<WorkspaceEdit> = client
            .request(<lsp_types::request::Rename as LspRequest>::METHOD, params)
            .await?;
        self.post(AppEvent::LspRename { request: req_id });
        Ok(r)
    }

    pub async fn format(
        &self,
        uri: Uri,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<Option<Vec<TextEdit>>> {
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            options: FormattingOptions {
                tab_size,
                insert_spaces,
                ..Default::default()
            },
            work_done_progress_params: Default::default(),
        };
        let r: Option<Vec<TextEdit>> = client
            .request(
                <lsp_types::request::Formatting as LspRequest>::METHOD,
                params,
            )
            .await?;
        Ok(r)
    }

    pub async fn document_symbol(&self, uri: Uri) -> Result<Option<DocumentSymbolResponse>> {
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let r: Option<DocumentSymbolResponse> = client
            .request(
                <lsp_types::request::DocumentSymbolRequest as LspRequest>::METHOD,
                params,
            )
            .await?;
        Ok(r)
    }

    pub async fn inlay_hint(&self, uri: Uri, range: Range) -> Result<Option<Vec<InlayHint>>> {
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = InlayHintParams {
            text_document: TextDocumentIdentifier { uri },
            range,
            work_done_progress_params: Default::default(),
        };
        let r: Option<Vec<InlayHint>> = client
            .request(
                <lsp_types::request::InlayHintRequest as LspRequest>::METHOD,
                params,
            )
            .await?;
        Ok(r)
    }

    pub async fn semantic_tokens_full(&self, uri: Uri) -> Result<Option<SemanticTokensResult>> {
        let Some(client) = self.any_client().cloned() else {
            return Ok(None);
        };
        let params = SemanticTokensParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let r: Option<SemanticTokensResult> = client
            .request(
                <lsp_types::request::SemanticTokensFullRequest as LspRequest>::METHOD,
                params,
            )
            .await?;
        Ok(r)
    }
}

// Silence unused-import warnings for items kept available to consumers.
#[allow(dead_code)]
fn _unused() -> Value {
    json!({})
}
