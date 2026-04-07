//! DAP wire protocol: Content-Length framed JSON messages.
//!
//! Same framing as LSP, different schema. We model the three message
//! kinds (request/response/event) but keep `arguments`/`body` as
//! `serde_json::Value` to avoid pulling in a heavy DAP types crate.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// A DAP protocol message. Tagged on the `type` field per the spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DapMessage {
    Request {
        seq: u64,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arguments: Option<Value>,
    },
    Response {
        seq: u64,
        request_seq: u64,
        command: String,
        success: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Event {
        seq: u64,
        event: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<Value>,
    },
}

/// Write one Content-Length framed DAP message.
pub async fn write_message<W: AsyncWrite + Unpin>(w: &mut W, msg: &DapMessage) -> Result<()> {
    let body = serde_json::to_vec(msg)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    w.write_all(header.as_bytes()).await?;
    w.write_all(&body).await?;
    w.flush().await?;
    Ok(())
}

/// Read one Content-Length framed DAP message. Returns `None` on clean EOF.
pub async fn read_message<R: AsyncRead + Unpin>(r: &mut R) -> Result<Option<DapMessage>> {
    // Parse headers byte-by-byte until \r\n\r\n.
    let mut header = Vec::with_capacity(64);
    loop {
        let mut b = [0u8; 1];
        if r.read(&mut b).await? == 0 {
            if header.is_empty() {
                return Ok(None);
            }
            bail!("unexpected eof in dap header");
        }
        header.push(b[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
        if header.len() > 64 * 1024 {
            bail!("dap header too large");
        }
    }
    let header_str = std::str::from_utf8(&header)?;
    let mut content_length: Option<usize> = None;
    for line in header_str.split("\r\n") {
        if line.is_empty() {
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| anyhow!("bad dap header line: {line}"))?;
        if k.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(v.trim().parse()?);
        }
    }
    let len = content_length.ok_or_else(|| anyhow!("missing Content-Length"))?;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    let msg: DapMessage = serde_json::from_slice(&buf)?;
    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::duplex;

    #[tokio::test]
    async fn framing_round_trip_request() {
        let (mut a, mut b) = duplex(4096);
        let msg = DapMessage::Request {
            seq: 1,
            command: "initialize".into(),
            arguments: Some(json!({"adapterID": "mock"})),
        };
        write_message(&mut a, &msg).await.unwrap();
        let got = read_message(&mut b).await.unwrap().unwrap();
        match got {
            DapMessage::Request {
                seq,
                command,
                arguments,
            } => {
                assert_eq!(seq, 1);
                assert_eq!(command, "initialize");
                assert_eq!(arguments, Some(json!({"adapterID": "mock"})));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn framing_round_trip_response_and_event() {
        let (mut a, mut b) = duplex(4096);
        let resp = DapMessage::Response {
            seq: 2,
            request_seq: 1,
            command: "initialize".into(),
            success: true,
            body: Some(json!({"supportsConfigurationDoneRequest": true})),
            message: None,
        };
        let evt = DapMessage::Event {
            seq: 3,
            event: "stopped".into(),
            body: Some(json!({"reason": "breakpoint", "threadId": 1})),
        };
        write_message(&mut a, &resp).await.unwrap();
        write_message(&mut a, &evt).await.unwrap();
        let r1 = read_message(&mut b).await.unwrap().unwrap();
        let r2 = read_message(&mut b).await.unwrap().unwrap();
        assert!(matches!(r1, DapMessage::Response { success: true, .. }));
        assert!(matches!(r2, DapMessage::Event { .. }));
    }

    #[tokio::test]
    async fn read_returns_none_on_clean_eof() {
        let (a, mut b) = duplex(64);
        drop(a);
        let got = read_message(&mut b).await.unwrap();
        assert!(got.is_none());
    }
}
