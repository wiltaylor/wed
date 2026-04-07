//! Command execution context.
//!
//! `CommandContext` bundles mutable references to the editor pieces a
//! command needs. We deliberately do NOT pass `&mut App` because the
//! registry itself lives on `App` and a callback would otherwise be
//! unable to borrow both at once.

use crate::app::AppEvent;
use crate::config::Config;
use crate::editor::Buffer;
use crate::input::EditorMode;
use crate::layout::LayoutState;
use tokio::sync::mpsc::UnboundedSender;

/// Records information about the most recent buffer-mutating change so
/// that `.` (dot-repeat) can replay it.
#[derive(Debug, Clone, Default)]
pub struct LastChange {
    pub kind: String,
    pub count: usize,
    pub inserted: String,
}

pub struct CommandContext<'a> {
    pub buffers: &'a mut Vec<Buffer>,
    pub layout: &'a mut LayoutState,
    pub mode: &'a mut EditorMode,
    pub config: &'a Config,
    pub event_tx: &'a UnboundedSender<AppEvent>,
    /// Set to `true` to request app shutdown.
    pub quit: &'a mut bool,
    pub count: usize,
}

impl<'a> CommandContext<'a> {
    pub fn new(
        buffers: &'a mut Vec<Buffer>,
        layout: &'a mut LayoutState,
        mode: &'a mut EditorMode,
        config: &'a Config,
        event_tx: &'a UnboundedSender<AppEvent>,
        quit: &'a mut bool,
    ) -> Self {
        Self {
            buffers,
            layout,
            mode,
            config,
            event_tx,
            quit,
            count: 1,
        }
    }
}

#[cfg(test)]
pub(crate) fn test_ctx(quit: &mut bool) -> CommandContext<'_> {
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    struct TestState {
        buffers: Vec<Buffer>,
        layout: LayoutState,
        mode: EditorMode,
        config: Config,
        tx: UnboundedSender<AppEvent>,
    }
    static STATE: Lazy<Mutex<Option<TestState>>> = Lazy::new(|| Mutex::new(None));
    {
        let mut g = STATE.lock().unwrap();
        if g.is_none() {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            // Keep rx alive forever to avoid send errors.
            Box::leak(Box::new(rx));
            *g = Some(TestState {
                buffers: Vec::new(),
                layout: LayoutState::default(),
                mode: EditorMode::Normal,
                config: Config::default(),
                tx,
            });
        }
    }
    // SAFETY: test scratch is process-global and tests touching it are
    // serial. We extend its lifetime to the borrow of `quit`.
    let state: &'static mut TestState = unsafe {
        let mut g = STATE.lock().unwrap();
        let p: *mut TestState = g.as_mut().unwrap() as *mut _;
        &mut *p
    };
    CommandContext {
        buffers: &mut state.buffers,
        layout: &mut state.layout,
        mode: &mut state.mode,
        config: &state.config,
        event_tx: &state.tx,
        quit,
        count: 1,
    }
}
