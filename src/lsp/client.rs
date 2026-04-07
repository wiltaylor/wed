//! Async LSP client speaking JSON-RPC 2.0 over a child process's stdio.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncWrite, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};

use crate::app::{AppEvent, ServerId};
use crate::lsp::protocol::{read_message, write_message};

type PendingMap = Arc<Mutex<HashMap<i32, oneshot::Sender<Value>>>>;

/// A pending `textDocument/didChange` queued for debounced delivery.
#[derive(Debug, Clone)]
pub struct DidChangeRequest {
    pub uri: lsp_types::Uri,
    pub version: i32,
    pub text: String,
}

/// A spawned LSP server. Cheap to clone via `Arc`.
pub struct LspClient {
    pub id: ServerId,
    pub name: String,
    next_id: AtomicI32,
    pending: PendingMap,
    writer: AsyncMutex<Box<dyn AsyncWrite + Send + Unpin>>,
    /// Notifications received from the server (method, params).
    pub notifications: Mutex<Option<mpsc::UnboundedReceiver<(String, Value)>>>,
    notif_tx: mpsc::UnboundedSender<(String, Value)>,
    /// Channel feeding the debounced `didChange` task. Only the most
    /// recent entry wins: rapid keystrokes collapse to a single notification.
    did_change_tx: Mutex<Option<mpsc::UnboundedSender<DidChangeRequest>>>,
    _child: Mutex<Option<Child>>,
}

impl LspClient {
    /// Spawn a server process.
    pub async fn spawn(
        id: ServerId,
        name: impl Into<String>,
        command: &str,
        args: &[String],
        event_tx: Option<mpsc::UnboundedSender<AppEvent>>,
    ) -> Result<Arc<Self>> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to spawn lsp server {command}"))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        // Drain stderr so the child doesn't block on a full pipe buffer.
        if let Some(stderr) = child.stderr.take() {
            let name_for_log = format!("lsp[{}]", id.0);
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader as TBufReader};
                let mut lines = TBufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::info!("{name_for_log} stderr: {line}");
                }
            });
        }
        let writer: Box<dyn AsyncWrite + Send + Unpin> = Box::new(stdin);
        let client = Self::with_io(id, name, writer, event_tx);
        let pending = client.pending.clone();
        let notif_tx = client.notif_tx.clone();
        let server_id = id;
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            while let Ok(msg) = read_message(&mut reader).await {
                Self::dispatch(&pending, &notif_tx, msg);
            }
            let _ = server_id;
        });
        client._child.lock().replace(child);
        Ok(Arc::new(client))
    }

    /// Construct a client around an arbitrary writer (used by tests).
    pub fn with_io(
        id: ServerId,
        name: impl Into<String>,
        writer: Box<dyn AsyncWrite + Send + Unpin>,
        _event_tx: Option<mpsc::UnboundedSender<AppEvent>>,
    ) -> Self {
        let (notif_tx, notif_rx) = mpsc::unbounded_channel();
        Self {
            id,
            name: name.into(),
            next_id: AtomicI32::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            writer: AsyncMutex::new(writer),
            notifications: Mutex::new(Some(notif_rx)),
            notif_tx,
            did_change_tx: Mutex::new(None),
            _child: Mutex::new(None),
        }
    }

    /// Queue a `textDocument/didChange`. Multiple calls within ~80ms are
    /// coalesced into a single notification carrying the latest state.
    pub fn queue_did_change(self: &Arc<Self>, req: DidChangeRequest) {
        // Lazily spawn the debounce task + channel on first use.
        let need_spawn = self.did_change_tx.lock().is_none();
        if need_spawn {
            let (tx, mut rx) = mpsc::unbounded_channel::<DidChangeRequest>();
            *self.did_change_tx.lock() = Some(tx);
            let client = Arc::clone(self);
            tokio::spawn(async move {
                use std::time::Duration;
                while let Some(first) = rx.recv().await {
                    let mut latest = first;
                    // Drain any further changes that arrive within 300ms of
                    // the last one so rapid typing becomes a single send.
                    // rust-analyzer cancels its in-flight diagnostic
                    // computation on each `didChange`, so sending less often
                    // dramatically improves how quickly errors show up.
                    loop {
                        match tokio::time::timeout(Duration::from_millis(300), rx.recv()).await
                        {
                            Ok(Some(m)) => latest = m,
                            _ => break,
                        }
                    }
                    use lsp_types::{
                        DidChangeTextDocumentParams, TextDocumentContentChangeEvent,
                        VersionedTextDocumentIdentifier,
                    };
                    let params = DidChangeTextDocumentParams {
                        text_document: VersionedTextDocumentIdentifier {
                            uri: latest.uri,
                            version: latest.version,
                        },
                        content_changes: vec![TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text: latest.text,
                        }],
                    };
                    let v = latest.version;
                    tracing::info!("lsp debounce send didChange v={v}");
                    if let Err(e) = client.notify("textDocument/didChange", params).await {
                        tracing::warn!("lsp did_change send failed: {e:#}");
                    }
                }
            });
        }
        if let Some(tx) = self.did_change_tx.lock().as_ref() {
            let _ = tx.send(req);
        }
    }

    /// Returns a clone of the pending map (for tests/manual reader loops).
    pub fn pending(&self) -> PendingMap {
        self.pending.clone()
    }

    /// Returns a sender for the notification channel.
    pub fn notif_sender(&self) -> mpsc::UnboundedSender<(String, Value)> {
        self.notif_tx.clone()
    }

    /// Dispatch a parsed message: response → resolve oneshot, notification → channel.
    pub fn dispatch(
        pending: &PendingMap,
        notif_tx: &mpsc::UnboundedSender<(String, Value)>,
        msg: Value,
    ) {
        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
            // Could still be a server→client request; treat anything with `method` as such.
            if msg.get("method").is_some() {
                if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                    let params = msg.get("params").cloned().unwrap_or(Value::Null);
                    let _ = notif_tx.send((method.to_string(), params));
                }
                return;
            }
            if let Some(tx) = pending.lock().remove(&(id as i32)) {
                let _ = tx.send(msg);
            }
        } else if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            let _ = notif_tx.send((method.to_string(), params));
        }
    }

    /// Send a request and await its typed response.
    pub async fn request<P, R>(&self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(id, tx);
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        {
            let mut w = self.writer.lock().await;
            write_message(&mut *w, &msg).await?;
        }
        let resp = rx.await.map_err(|_| anyhow!("lsp client dropped"))?;
        if let Some(err) = resp.get("error") {
            return Err(anyhow!("lsp error: {err}"));
        }
        let result = resp.get("result").cloned().unwrap_or(Value::Null);
        Ok(serde_json::from_value(result)?)
    }

    /// Send a notification (fire-and-forget).
    pub async fn notify<P: Serialize>(&self, method: &str, params: P) -> Result<()> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let mut w = self.writer.lock().await;
        write_message(&mut *w, &msg).await
    }
}

impl std::fmt::Debug for LspClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspClient")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

// Allow ChildStdin to be coerced into Box<dyn AsyncWrite>.
#[allow(dead_code)]
fn _assert_child_stdin(_: ChildStdin) {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{duplex, BufReader};

    #[tokio::test]
    async fn request_response_correlation() {
        let (a, b) = duplex(4096);
        let (a_read, a_write) = tokio::io::split(a);
        let (b_read, b_write) = tokio::io::split(b);

        let writer: Box<dyn AsyncWrite + Send + Unpin> = Box::new(a_write);
        let client = Arc::new(LspClient::with_io(ServerId(1), "mock", writer, None));

        // Spawn the read loop manually using the duplex's other half.
        let pending = client.pending();
        let notif_tx = client.notif_sender();
        tokio::spawn(async move {
            let mut reader = BufReader::new(a_read);
            while let Ok(msg) = read_message(&mut reader).await {
                LspClient::dispatch(&pending, &notif_tx, msg);
            }
        });

        // Mock server: read the outgoing request, then send a response.
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(b_read);
            let req = read_message(&mut reader).await.unwrap();
            let id = req.get("id").and_then(|v| v.as_i64()).unwrap();
            let mut w = b_write;
            let resp = json!({"jsonrpc":"2.0","id":id,"result":{"ok":true,"n":7}});
            write_message(&mut w, &resp).await.unwrap();
        });

        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct R {
            ok: bool,
            n: i32,
        }
        let r: R = client.request("test/method", json!({})).await.unwrap();
        assert_eq!(r, R { ok: true, n: 7 });
        server.await.unwrap();
    }
}
