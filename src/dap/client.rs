//! DAP client. Spawns a debug adapter (stdio or tcp), runs a read loop,
//! and correlates responses to in-flight requests by sequence number.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};

use crate::app::{AppEvent, SessionId};
use crate::dap::protocol::{read_message, write_message, DapMessage};

/// A completed response from the adapter.
#[derive(Debug, Clone)]
pub struct DapResponse {
    pub command: String,
    pub success: bool,
    pub body: Option<Value>,
    pub message: Option<String>,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<DapResponse>>>>;

/// A connected DAP client. Owns the writer half; the reader runs in a
/// background task that resolves pending requests and forwards events.
pub struct DapClient {
    pub id: SessionId,
    pub name: String,
    seq: AtomicU64,
    writer: AsyncMutex<Box<dyn AsyncWrite + Unpin + Send>>,
    pending: Pending,
}

impl std::fmt::Debug for DapClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DapClient")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl DapClient {
    /// Create a client from arbitrary AsyncRead/AsyncWrite halves.
    pub fn from_split(
        id: SessionId,
        name: String,
        reader: Box<dyn AsyncRead + Unpin + Send>,
        writer: Box<dyn AsyncWrite + Unpin + Send>,
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Self {
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let pending_for_reader = pending.clone();
        let session = id;
        tokio::spawn(async move {
            let mut r = reader;
            loop {
                match read_message(&mut r).await {
                    Ok(Some(msg)) => {
                        dispatch(msg, &pending_for_reader, session, &event_tx);
                    }
                    Ok(None) => {
                        let _ = event_tx.send(AppEvent::DapTerminated { session });
                        break;
                    }
                    Err(_) => {
                        let _ = event_tx.send(AppEvent::DapTerminated { session });
                        break;
                    }
                }
            }
        });

        Self {
            id,
            name,
            seq: AtomicU64::new(1),
            writer: AsyncMutex::new(writer),
            pending,
        }
    }

    /// Spawn a debug adapter as a child process and connect over its stdio.
    pub async fn spawn_stdio(
        id: SessionId,
        name: String,
        program: &str,
        args: &[String],
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to capture adapter stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture adapter stdout"))?;
        // Detach the child; the read loop will signal termination on EOF.
        tokio::spawn(async move {
            let _ = child.wait().await;
        });
        Ok(Self::from_split(
            id,
            name,
            Box::new(stdout),
            Box::new(stdin),
            event_tx,
        ))
    }

    /// Connect to a DAP adapter listening on a TCP host:port.
    pub async fn connect_tcp(
        id: SessionId,
        name: String,
        host: &str,
        port: u16,
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Result<Self> {
        let stream = TcpStream::connect((host, port)).await?;
        let (r, w) = stream.into_split();
        Ok(Self::from_split(
            id,
            name,
            Box::new(r),
            Box::new(w),
            event_tx,
        ))
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a request and await its response.
    pub async fn request(&self, command: &str, arguments: Option<Value>) -> Result<DapResponse> {
        let seq = self.next_seq();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(seq, tx);
        let msg = DapMessage::Request {
            seq,
            command: command.to_string(),
            arguments,
        };
        {
            let mut w = self.writer.lock().await;
            if let Err(e) = write_message(&mut *w, &msg).await {
                self.pending.lock().remove(&seq);
                return Err(e);
            }
        }
        rx.await
            .map_err(|_| anyhow!("dap response channel dropped"))
    }

    /// Send a request without awaiting a response (fire and forget).
    pub async fn notify(&self, command: &str, arguments: Option<Value>) -> Result<()> {
        let seq = self.next_seq();
        let msg = DapMessage::Request {
            seq,
            command: command.to_string(),
            arguments,
        };
        let mut w = self.writer.lock().await;
        write_message(&mut *w, &msg).await
    }

    /// Test hook: register a pending request slot directly.
    #[cfg(test)]
    pub(crate) fn register_pending(&self, seq: u64) -> oneshot::Receiver<DapResponse> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(seq, tx);
        rx
    }

    #[cfg(test)]
    pub(crate) fn pending_clone(&self) -> Pending {
        self.pending.clone()
    }
}

fn dispatch(
    msg: DapMessage,
    pending: &Pending,
    session: SessionId,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match msg {
        DapMessage::Response {
            request_seq,
            command,
            success,
            body,
            message,
            ..
        } => {
            if let Some(tx) = pending.lock().remove(&request_seq) {
                let _ = tx.send(DapResponse {
                    command,
                    success,
                    body,
                    message,
                });
            }
        }
        DapMessage::Event { event, body, .. } => {
            let app_event = match event.as_str() {
                "stopped" => Some(AppEvent::DapStopped { session }),
                "continued" => Some(AppEvent::DapContinued { session }),
                "terminated" | "exited" => Some(AppEvent::DapTerminated { session }),
                "breakpoint" => Some(AppEvent::DapBreakpointVerified { session }),
                "output" => {
                    let text = body
                        .as_ref()
                        .and_then(|b| b.get("output"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(AppEvent::DapOutput { session, text })
                }
                _ => None,
            };
            if let Some(e) = app_event {
                let _ = event_tx.send(e);
            }
        }
        DapMessage::Request { .. } => {
            // Reverse requests (e.g. runInTerminal) are not handled yet.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dap::protocol::write_message;
    use serde_json::json;
    use tokio::io::{duplex, sink};

    #[tokio::test]
    async fn sequence_correlation_resolves_oneshot() {
        // Wire: client reads from `cr` end, we write fake responses into `sw` end.
        let (sw, cr) = duplex(4096);
        let writer = Box::new(sink());
        let (etx, _erx) = mpsc::unbounded_channel();
        let client = DapClient::from_split(SessionId(0), "mock".into(), Box::new(cr), writer, etx);

        // Pretend we sent request seq=42.
        let rx = client.register_pending(42);

        // Push a fake response from the "adapter" side.
        let mut sw = sw;
        let resp = DapMessage::Response {
            seq: 1,
            request_seq: 42,
            command: "initialize".into(),
            success: true,
            body: Some(json!({"ok": true})),
            message: None,
        };
        write_message(&mut sw, &resp).await.unwrap();

        let got = rx.await.expect("oneshot resolved");
        assert!(got.success);
        assert_eq!(got.command, "initialize");
        assert_eq!(got.body, Some(json!({"ok": true})));
        // The pending map should be drained.
        assert!(client.pending_clone().lock().is_empty());
    }
}
