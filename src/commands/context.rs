//! Command execution context.
//!
//! Carries mutable references to the pieces of editor state a command
//! needs to mutate. Other subsystems (LSP, layout, ...) can be threaded
//! in additively as needed.

use crate::editor::Buffer;
use crate::editor::Cursor;
use crate::input::pending::PendingState;
use crate::input::EditorMode;

/// Records information about the most recent buffer-mutating change so
/// that `.` (dot-repeat) can replay it.
#[derive(Debug, Clone, Default)]
pub struct LastChange {
    /// Stringified description (operator + motion + count + inserted text).
    pub kind: String,
    pub count: usize,
    /// Text typed during the change (for insert-mode replays).
    pub inserted: String,
}

pub struct CommandContext<'a> {
    pub buffer: &'a mut Buffer,
    pub cursor: &'a mut Cursor,
    pub mode: &'a mut EditorMode,
    pub pending: &'a mut PendingState,
    pub last_change: &'a mut LastChange,
    pub count: usize,
}

impl<'a> CommandContext<'a> {
    pub fn new(
        buffer: &'a mut Buffer,
        cursor: &'a mut Cursor,
        mode: &'a mut EditorMode,
        pending: &'a mut PendingState,
        last_change: &'a mut LastChange,
    ) -> Self {
        Self { buffer, cursor, mode, pending, last_change, count: 1 }
    }
}
