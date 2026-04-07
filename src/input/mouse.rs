//! Mouse event routing.
//!
//! Maps a `crossterm::event::MouseEvent` to a logical region of the UI
//! (tabline, gutter, buffer area, sidebar, statusline) using a set of
//! `LayoutRects` provided by the renderer. The function is pure: callers
//! pass in the rects rather than reading them from `App`.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

/// Rectangle in screen cells (zero-based).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Last-known UI rectangles, populated by the renderer.
#[derive(Debug, Clone, Default)]
pub struct LayoutRects {
    pub tabline: Rect,
    pub statusline: Rect,
    pub left_sidebar: Rect,
    pub right_sidebar: Rect,
    /// Per-tab tab labels in the tabline (x ranges).
    pub tab_labels: Vec<Rect>,
    /// Buffer (editor) area for each visible view.
    pub views: Vec<ViewRect>,
}

#[derive(Debug, Clone, Default)]
pub struct ViewRect {
    /// Full view rect including gutter.
    pub area: Rect,
    /// The line-number / sign gutter sub-rect.
    pub gutter: Rect,
    /// Remaining buffer text area (to the right of the gutter).
    pub text: Rect,
}

/// A logical mouse action produced by routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseAction {
    None,
    TabGoto(usize),
    ToggleBreakpoint { view: usize, line: u16 },
    CursorMove { view: usize, line: u16, col: u16 },
    SidebarLeftClick { line: u16 },
    SidebarRightClick { line: u16 },
    StatuslineClick,
    ScrollUp { view: usize },
    ScrollDown { view: usize },
}

/// Pure routing function: takes the rects and a mouse event, returns
/// a logical action. Used by `handle_mouse` and tests.
pub fn route_mouse(rects: &LayoutRects, ev: MouseEvent) -> MouseAction {
    let (x, y) = (ev.column, ev.row);
    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if rects.tabline.contains(x, y) {
                for (i, r) in rects.tab_labels.iter().enumerate() {
                    if r.contains(x, y) {
                        return MouseAction::TabGoto(i);
                    }
                }
                return MouseAction::None;
            }
            if rects.statusline.contains(x, y) {
                return MouseAction::StatuslineClick;
            }
            if rects.left_sidebar.contains(x, y) {
                return MouseAction::SidebarLeftClick {
                    line: y - rects.left_sidebar.y,
                };
            }
            if rects.right_sidebar.contains(x, y) {
                return MouseAction::SidebarRightClick {
                    line: y - rects.right_sidebar.y,
                };
            }
            for (i, v) in rects.views.iter().enumerate() {
                if v.gutter.contains(x, y) {
                    return MouseAction::ToggleBreakpoint {
                        view: i,
                        line: y - v.gutter.y,
                    };
                }
                if v.text.contains(x, y) {
                    return MouseAction::CursorMove {
                        view: i,
                        line: y - v.text.y,
                        col: x - v.text.x,
                    };
                }
            }
            MouseAction::None
        }
        MouseEventKind::ScrollUp => {
            for (i, v) in rects.views.iter().enumerate() {
                if v.area.contains(x, y) {
                    return MouseAction::ScrollUp { view: i };
                }
            }
            MouseAction::None
        }
        MouseEventKind::ScrollDown => {
            for (i, v) in rects.views.iter().enumerate() {
                if v.area.contains(x, y) {
                    return MouseAction::ScrollDown { view: i };
                }
            }
            MouseAction::None
        }
        _ => MouseAction::None,
    }
}

/// Top-level mouse handler. Currently just resolves the action via
/// `route_mouse` against the supplied rects and returns it; the caller
/// (the App event loop) is responsible for dispatching the action to
/// the command registry.
pub fn handle_mouse(rects: &LayoutRects, ev: MouseEvent) -> MouseAction {
    route_mouse(rects, ev)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

    fn ev(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn tabline_click_routes_to_tab_goto() {
        let rects = LayoutRects {
            tabline: Rect::new(0, 0, 80, 1),
            tab_labels: vec![Rect::new(0, 0, 10, 1), Rect::new(10, 0, 10, 1)],
            ..Default::default()
        };
        let action = route_mouse(&rects, ev(MouseEventKind::Down(MouseButton::Left), 12, 0));
        assert_eq!(action, MouseAction::TabGoto(1));
    }

    #[test]
    fn buffer_click_routes_to_cursor_move() {
        let rects = LayoutRects {
            views: vec![ViewRect {
                area: Rect::new(0, 1, 80, 20),
                gutter: Rect::new(0, 1, 4, 20),
                text: Rect::new(4, 1, 76, 20),
            }],
            ..Default::default()
        };
        let action = route_mouse(&rects, ev(MouseEventKind::Down(MouseButton::Left), 10, 5));
        assert_eq!(
            action,
            MouseAction::CursorMove {
                view: 0,
                line: 4,
                col: 6
            }
        );
    }

    #[test]
    fn gutter_click_toggles_breakpoint() {
        let rects = LayoutRects {
            views: vec![ViewRect {
                area: Rect::new(0, 1, 80, 20),
                gutter: Rect::new(0, 1, 4, 20),
                text: Rect::new(4, 1, 76, 20),
            }],
            ..Default::default()
        };
        let action = route_mouse(&rects, ev(MouseEventKind::Down(MouseButton::Left), 2, 5));
        assert_eq!(action, MouseAction::ToggleBreakpoint { view: 0, line: 4 });
    }
}
