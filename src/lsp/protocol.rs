//! JSON-RPC 2.0 framing for the Language Server Protocol.
//!
//! Messages are framed with `Content-Length: N\r\n\r\n<bytes>` headers.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// Read a single LSP message from `reader`. Returns the parsed JSON value.
pub async fn read_message<R>(reader: &mut BufReader<R>) -> Result<Value>
where
    R: AsyncRead + Unpin,
{
    let mut content_length: Option<usize> = None;
    let mut header = String::new();
    loop {
        header.clear();
        let n = reader.read_line(&mut header).await?;
        if n == 0 {
            return Err(anyhow!("eof while reading lsp headers"));
        }
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(rest.trim().parse().context("invalid Content-Length")?);
        }
    }
    let len = content_length.ok_or_else(|| anyhow!("missing Content-Length header"))?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    let value: Value = serde_json::from_slice(&buf).context("invalid json body")?;
    Ok(value)
}

/// Write a single LSP message to `writer`.
pub async fn write_message<W>(writer: &mut W, msg: &Value) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let body = serde_json::to_vec(msg)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::duplex;

    #[tokio::test]
    async fn round_trip() {
        let (a, b) = duplex(4096);
        let (_ar, mut aw) = tokio::io::split(a);
        let (br, _bw) = tokio::io::split(b);
        let msg = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"x":42}});
        write_message(&mut aw, &msg).await.unwrap();
        drop(aw);
        let mut reader = BufReader::new(br);
        let got = read_message(&mut reader).await.unwrap();
        assert_eq!(got, msg);
    }

    #[tokio::test]
    async fn round_trip_multiple() {
        let (a, b) = duplex(8192);
        let (_ar, mut aw) = tokio::io::split(a);
        let (br, _bw) = tokio::io::split(b);
        let m1 = json!({"jsonrpc":"2.0","id":1,"result":{}});
        let m2 = json!({"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"hi"}});
        write_message(&mut aw, &m1).await.unwrap();
        write_message(&mut aw, &m2).await.unwrap();
        drop(aw);
        let mut reader = BufReader::new(br);
        assert_eq!(read_message(&mut reader).await.unwrap(), m1);
        assert_eq!(read_message(&mut reader).await.unwrap(), m2);
    }
}
