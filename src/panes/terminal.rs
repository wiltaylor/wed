use crate::layout::Pane;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use parking_lot::Mutex;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::Write;
use std::sync::Arc;

/// Embedded terminal pane backed by `portable-pty`.
pub struct TerminalPane {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    _child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    pub buffer: Arc<Mutex<Vec<u8>>>,
    pub size: PtySize,
}

impl Default for TerminalPane {
    fn default() -> Self {
        Self::spawn_default_shell().expect("failed to spawn pty")
    }
}

impl TerminalPane {
    /// Spawn the user's `$SHELL` (or `cmd.exe` on Windows) in a new pty.
    pub fn spawn_default_shell() -> anyhow::Result<Self> {
        let shell = if cfg!(windows) {
            std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string())
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        };
        Self::spawn(&shell)
    }

    pub fn spawn(program: &str) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let size = PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 };
        let pair = pty_system.openpty(size)?;
        let cmd = CommandBuilder::new(program);
        let child = pair.slave.spawn_command(cmd)?;
        // The slave is no longer needed by us; the child owns the slave end.
        drop(pair.slave);

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = Arc::clone(&buffer);
        std::thread::spawn(move || {
            let mut tmp = [0u8; 4096];
            loop {
                match reader.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => buf_clone.lock().extend_from_slice(&tmp[..n]),
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            _child: Mutex::new(child),
            buffer,
            size,
        })
    }

    /// Resize the underlying pty (call from `render` when area changes).
    pub fn resize(&mut self, cols: u16, rows: u16) -> anyhow::Result<()> {
        if cols == self.size.cols && rows == self.size.rows {
            return Ok(());
        }
        let new = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        self.master.lock().resize(new)?;
        self.size = new;
        Ok(())
    }

    /// Write raw bytes to the pty master.
    pub fn write_input(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut w = self.writer.lock();
        w.write_all(bytes)?;
        w.flush()
    }

    /// Snapshot the read buffer for rendering.
    pub fn snapshot(&self) -> Vec<u8> { self.buffer.lock().clone() }

    fn key_to_bytes(key: KeyEvent) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    let lc = c.to_ascii_lowercase();
                    if ('a'..='z').contains(&lc) {
                        out.push((lc as u8) - b'a' + 1);
                    } else {
                        out.push(c as u8);
                    }
                } else {
                    let mut tmp = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
                }
            }
            KeyCode::Enter => out.push(b'\r'),
            KeyCode::Backspace => out.push(0x7f),
            KeyCode::Tab => out.push(b'\t'),
            KeyCode::Esc => out.push(0x1b),
            KeyCode::Left => out.extend_from_slice(b"\x1b[D"),
            KeyCode::Right => out.extend_from_slice(b"\x1b[C"),
            KeyCode::Up => out.extend_from_slice(b"\x1b[A"),
            KeyCode::Down => out.extend_from_slice(b"\x1b[B"),
            _ => return None,
        }
        Some(out)
    }
}

#[async_trait]
impl Pane for TerminalPane {
    fn name(&self) -> &str { "terminal" }
    fn handle_key(&mut self, key: KeyEvent) {
        if let Some(bytes) = Self::key_to_bytes(key) {
            let _ = self.write_input(&bytes);
        }
    }
}
