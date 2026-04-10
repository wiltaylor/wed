//! Bottom-panel pane that runs a `just` recipe and streams its output.

use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use parking_lot::Mutex;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::app::AppEvent;
use crate::layout::Pane;

pub struct JustPane {
    lines: Arc<Mutex<Vec<String>>>,
    running: Arc<AtomicBool>,
    /// `None` while running, `Some(true)` on success, `Some(false)` on failure.
    success: Arc<Mutex<Option<bool>>>,
    recipe_name: String,
    scroll: usize,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    /// Cancel flag for the previous run's reader threads.
    cancel: Arc<AtomicBool>,
    /// Child process handle for killing on re-run.
    child: Arc<Mutex<Option<Child>>>,
}

impl JustPane {
    pub fn new(event_tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        Self {
            lines: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
            success: Arc::new(Mutex::new(None)),
            recipe_name: String::new(),
            scroll: 0,
            event_tx,
            cancel: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
        }
    }

    /// Start (or restart) running a just recipe.
    pub fn run(&mut self, namepath: &str) {
        // Signal previous run to stop reading.
        self.cancel.store(true, Ordering::Relaxed);
        // Kill previous child if still running.
        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Reset state.
        self.recipe_name = namepath.to_string();
        self.scroll = 0;
        self.lines.lock().clear();
        *self.success.lock() = None;
        self.running.store(true, Ordering::Relaxed);

        // New cancel flag for this run.
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();

        let mut child = match Command::new("just")
            .arg("--color=always")
            .arg(namepath)
            .env("FORCE_COLOR", "1")
            .env("CLICOLOR_FORCE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                self.lines.lock().push(format!("failed to spawn just: {e}"));
                self.running.store(false, Ordering::Relaxed);
                *self.success.lock() = Some(false);
                return;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        *self.child.lock() = Some(child);

        let lines = Arc::clone(&self.lines);
        let running = Arc::clone(&self.running);
        let success = Arc::clone(&self.success);
        let child_handle = Arc::clone(&self.child);
        let tx = self.event_tx.clone();
        let cancel2 = cancel.clone();

        // Spawn stdout reader.
        let lines_out = Arc::clone(&lines);
        let tx_out = tx.clone();
        let cancel_out = cancel.clone();
        let stdout_thread = stdout.map(|s| {
            std::thread::spawn(move || {
                let reader = BufReader::new(s);
                for line in reader.lines() {
                    if cancel_out.load(Ordering::Relaxed) {
                        break;
                    }
                    match line {
                        Ok(l) => {
                            lines_out.lock().push(l);
                            let _ = tx_out.send(AppEvent::Render);
                        }
                        Err(_) => break,
                    }
                }
            })
        });

        // Spawn stderr reader.
        let lines_err = Arc::clone(&lines);
        let tx_err = tx.clone();
        let stderr_thread = stderr.map(|s| {
            std::thread::spawn(move || {
                let reader = BufReader::new(s);
                for line in reader.lines() {
                    if cancel2.load(Ordering::Relaxed) {
                        break;
                    }
                    match line {
                        Ok(l) => {
                            lines_err.lock().push(l);
                            let _ = tx_err.send(AppEvent::Render);
                        }
                        Err(_) => break,
                    }
                }
            })
        });

        // Spawn waiter thread that collects exit status.
        std::thread::spawn(move || {
            if let Some(t) = stdout_thread {
                let _ = t.join();
            }
            if let Some(t) = stderr_thread {
                let _ = t.join();
            }
            let exit_ok = child_handle
                .lock()
                .as_mut()
                .and_then(|c| c.wait().ok())
                .map(|s| s.success())
                .unwrap_or(false);
            *success.lock() = Some(exit_ok);
            running.store(false, Ordering::Relaxed);
            let _ = tx.send(AppEvent::Render);
        });
    }

    fn title_string(&self) -> String {
        let name = if self.recipe_name.is_empty() {
            "just"
        } else {
            &self.recipe_name
        };
        if self.running.load(Ordering::Relaxed) {
            format!("just: {name} …")
        } else {
            match *self.success.lock() {
                Some(true) => format!("just: {name} ✓"),
                Some(false) => format!("just: {name} ✗"),
                None => format!("just: {name}"),
            }
        }
    }
}

#[async_trait]
impl Pane for JustPane {
    fn name(&self) -> &str {
        "just"
    }

    fn dynamic_title(&self) -> Option<String> {
        Some(self.title_string())
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let lines = self.lines.lock();
        let total = lines.len();
        let visible = area.height as usize;

        // Auto-scroll to bottom while running, otherwise respect manual scroll.
        let start = if self.running.load(Ordering::Relaxed) {
            total.saturating_sub(visible)
        } else {
            self.scroll.min(total.saturating_sub(visible))
        };

        let display_lines: Vec<Line> = lines
            .iter()
            .skip(start)
            .take(visible)
            .map(|l| parse_ansi_line(l))
            .collect();

        frame.render_widget(Paragraph::new(display_lines), area);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let total = self.lines.lock().len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.scroll > 0 {
                    self.scroll -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1).min(total.saturating_sub(1));
            }
            KeyCode::Char('G') => {
                self.scroll = total;
            }
            KeyCode::Char('g') => {
                self.scroll = 0;
            }
            _ => {}
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// Parse a single line containing ANSI SGR escape sequences into a ratatui `Line`
/// of styled `Span`s.
fn parse_ansi_line(input: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style = Style::default().fg(Color::Gray);
    let mut pos = 0;
    let bytes = input.as_bytes();

    while pos < bytes.len() {
        // Look for ESC[
        if bytes[pos] == 0x1b && pos + 1 < bytes.len() && bytes[pos + 1] == b'[' {
            // Find the end of the sequence (a letter)
            let seq_start = pos + 2;
            let mut seq_end = seq_start;
            while seq_end < bytes.len() && (bytes[seq_end] == b';' || bytes[seq_end].is_ascii_digit()) {
                seq_end += 1;
            }
            if seq_end < bytes.len() && bytes[seq_end] == b'm' {
                // It's an SGR sequence — parse the params.
                let params = &input[seq_start..seq_end];
                style = apply_sgr(style, params);
                pos = seq_end + 1;
                continue;
            }
            // Not an SGR sequence — skip ESC[ and the terminator as raw text.
            if seq_end < bytes.len() {
                pos = seq_end + 1;
            } else {
                pos = seq_end;
            }
            continue;
        }
        // Accumulate plain text until the next ESC.
        let text_start = pos;
        while pos < bytes.len() && bytes[pos] != 0x1b {
            pos += 1;
        }
        if text_start < pos {
            spans.push(Span::styled(input[text_start..pos].to_string(), style));
        }
    }

    if spans.is_empty() {
        Line::from(Span::styled(String::new(), Style::default()))
    } else {
        Line::from(spans)
    }
}

/// Apply SGR (Select Graphic Rendition) parameters to a style.
/// Handles: reset, bold, dim, italic, underline, standard colors (30-37, 40-47),
/// bright colors (90-97, 100-107), 256-color (38;5;n / 48;5;n).
fn apply_sgr(mut style: Style, params: &str) -> Style {
    if params.is_empty() {
        return Style::default().fg(Color::Gray);
    }
    let mut iter = params.split(';').peekable();
    while let Some(p) = iter.next() {
        match p {
            "0" => style = Style::default().fg(Color::Gray),
            "1" => style = style.add_modifier(Modifier::BOLD),
            "2" => style = style.add_modifier(Modifier::DIM),
            "3" => style = style.add_modifier(Modifier::ITALIC),
            "4" => style = style.add_modifier(Modifier::UNDERLINED),
            "22" => style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            "23" => style = style.remove_modifier(Modifier::ITALIC),
            "24" => style = style.remove_modifier(Modifier::UNDERLINED),
            // Standard foreground colors
            "30" => style = style.fg(Color::Black),
            "31" => style = style.fg(Color::Red),
            "32" => style = style.fg(Color::Green),
            "33" => style = style.fg(Color::Yellow),
            "34" => style = style.fg(Color::Blue),
            "35" => style = style.fg(Color::Magenta),
            "36" => style = style.fg(Color::Cyan),
            "37" => style = style.fg(Color::White),
            "39" => style = style.fg(Color::Gray), // default fg
            // Standard background colors
            "40" => style = style.bg(Color::Black),
            "41" => style = style.bg(Color::Red),
            "42" => style = style.bg(Color::Green),
            "43" => style = style.bg(Color::Yellow),
            "44" => style = style.bg(Color::Blue),
            "45" => style = style.bg(Color::Magenta),
            "46" => style = style.bg(Color::Cyan),
            "47" => style = style.bg(Color::White),
            "49" => style = style.bg(Color::Reset), // default bg
            // Bright foreground colors
            "90" => style = style.fg(Color::DarkGray),
            "91" => style = style.fg(Color::LightRed),
            "92" => style = style.fg(Color::LightGreen),
            "93" => style = style.fg(Color::LightYellow),
            "94" => style = style.fg(Color::LightBlue),
            "95" => style = style.fg(Color::LightMagenta),
            "96" => style = style.fg(Color::LightCyan),
            "97" => style = style.fg(Color::White),
            // Bright background colors
            "100" => style = style.bg(Color::DarkGray),
            "101" => style = style.bg(Color::LightRed),
            "102" => style = style.bg(Color::LightGreen),
            "103" => style = style.bg(Color::LightYellow),
            "104" => style = style.bg(Color::LightBlue),
            "105" => style = style.bg(Color::LightMagenta),
            "106" => style = style.bg(Color::LightCyan),
            "107" => style = style.bg(Color::White),
            // 256-color: 38;5;n (fg) and 48;5;n (bg)
            "38" => {
                if iter.peek().map(|s| *s) == Some("5") {
                    iter.next(); // consume "5"
                    if let Some(n) = iter.next().and_then(|s| s.parse::<u8>().ok()) {
                        style = style.fg(Color::Indexed(n));
                    }
                }
            }
            "48" => {
                if iter.peek().map(|s| *s) == Some("5") {
                    iter.next(); // consume "5"
                    if let Some(n) = iter.next().and_then(|s| s.parse::<u8>().ok()) {
                        style = style.bg(Color::Indexed(n));
                    }
                }
            }
            _ => {} // ignore unknown codes
        }
    }
    style
}
