pub mod command_line_ui;
pub mod editor_view;
pub mod highlight_render;
pub mod popup;
pub mod sidebar_render;
pub mod statusline;
pub mod tabline;

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::App;
use crate::input::EditorMode;
use crate::layout::View;

/// Adjust `view.scroll.0` so the cursor row stays within the visible
/// window, with a small scroll-off margin top and bottom.
pub fn clamp_scroll_to_cursor(view: &mut View, area_height: usize) {
    if area_height == 0 {
        return;
    }
    let scroll_off = 3.min(area_height / 2);
    let cursor_row = view.cursor.0;
    let top = view.scroll.0;
    // Cursor above viewport (with margin)
    if cursor_row < top + scroll_off {
        view.scroll.0 = cursor_row.saturating_sub(scroll_off);
    }
    // Cursor below viewport (with margin)
    let bottom = top + area_height;
    if cursor_row + scroll_off + 1 > bottom {
        view.scroll.0 = cursor_row + scroll_off + 1 - area_height;
    }
}

/// Top-level renderer. Lays out tabline / sidebars / editor / statusline / command-line / popups.
pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let size = frame.area();
    if size.width == 0 || size.height == 0 {
        return;
    }

    let show_tabline = app.config.ui.tabline && !app.layout.tabs.is_empty();
    let show_status = app.config.ui.statusline;
    let in_cmdline = matches!(app.mode, EditorMode::Command | EditorMode::Search)
        || app.status_message.is_some();

    // Reserve top row for tabline.
    let mut y = size.y;
    let mut h = size.height;

    let tab_rect = if show_tabline && h > 0 {
        let r = Rect {
            x: size.x,
            y,
            width: size.width,
            height: 1,
        };
        y += 1;
        h = h.saturating_sub(1);
        Some(r)
    } else {
        None
    };

    // Reserve bottom row(s) for statusline + command-line.
    let cmdline_rect = if in_cmdline && h > 0 {
        let r = Rect {
            x: size.x,
            y: y + h - 1,
            width: size.width,
            height: 1,
        };
        h = h.saturating_sub(1);
        Some(r)
    } else {
        None
    };

    let status_rect = if show_status && h > 0 {
        let r = Rect {
            x: size.x,
            y: y + h - 1,
            width: size.width,
            height: 1,
        };
        h = h.saturating_sub(1);
        Some(r)
    } else {
        None
    };

    // Sidebars consume columns from the middle band.
    let middle_y = y;
    let middle_h = h;
    let mut middle_x = size.x;
    let mut middle_w = size.width;

    let left_w = if app.layout.left_sidebar.open {
        app.layout.left_sidebar.width.min(middle_w)
    } else {
        0
    };
    let right_w = if app.layout.right_sidebar.open {
        app.layout
            .right_sidebar
            .width
            .min(middle_w.saturating_sub(left_w))
    } else {
        0
    };

    let left_rect = if left_w > 0 {
        let r = Rect {
            x: middle_x,
            y: middle_y,
            width: left_w,
            height: middle_h,
        };
        middle_x += left_w;
        middle_w -= left_w;
        Some(r)
    } else {
        None
    };

    let right_rect = if right_w > 0 {
        let r = Rect {
            x: middle_x + middle_w - right_w,
            y: middle_y,
            width: right_w,
            height: middle_h,
        };
        middle_w -= right_w;
        Some(r)
    } else {
        None
    };

    let editor_rect = Rect {
        x: middle_x,
        y: middle_y,
        width: middle_w,
        height: middle_h,
    };

    // Tabline
    if let Some(r) = tab_rect {
        tabline::render(frame, app, r);
    } else {
        app.last_tab_rects.clear();
    }

    // Sidebars
    if let Some(r) = left_rect {
        sidebar_render::render(frame, &app.layout.left_sidebar, r, "Explorer");
    }
    if let Some(r) = right_rect {
        sidebar_render::render(frame, &app.layout.right_sidebar, r, "Outline");
    }

    // Editor: layout the active tab's split tree.
    app.last_editor_rect = editor_rect;
    app.last_editor_view_rects.clear();
    if editor_rect.width > 0 && editor_rect.height > 0 {
        // First pass: compute rects and clamp each view's scroll so the
        // cursor stays inside the visible area.
        if let Some(tab) = app.layout.active_tab_mut() {
            let leaves = tab.root.layout_rects(editor_rect);
            for (vid, rect) in &leaves {
                if let Some(view) = tab.root.find_mut(*vid) {
                    clamp_scroll_to_cursor(view, rect.height as usize);
                }
            }
        }
        // Snapshot the layout so we can free the immutable borrow on
        // `app.layout` before calling `editor_view::render`, which needs
        // `&mut App` for the highlight engine.
        let snapshot: Option<(Vec<(crate::app::ViewId, Rect)>, crate::app::ViewId, Vec<(crate::app::ViewId, crate::layout::View)>)> = app
            .layout
            .active_tab()
            .map(|tab| {
                let leaves = tab.root.layout_rects(editor_rect);
                let views: Vec<_> = leaves
                    .iter()
                    .filter_map(|(vid, _)| tab.root.find(*vid).map(|v| (*vid, v.clone())))
                    .collect();
                (leaves, tab.active_view, views)
            });
        if let Some((leaves, active_view, views)) = snapshot {
            for (vid, view) in &views {
                if let Some((_, rect)) = leaves.iter().find(|(v, _)| v == vid) {
                    let is_active = *vid == active_view;
                    editor_view::render(frame, app, view, *rect, is_active);
                }
            }
            app.last_editor_view_rects = leaves;
        }
    }
    app.last_left_sidebar_rect = left_rect.unwrap_or_default();

    // Statusline (overlaid by command line when in command/search mode).
    if let Some(r) = status_rect {
        statusline::render(frame, app, r);
    }
    if let Some(r) = cmdline_rect {
        command_line_ui::render(frame, app, r);
    }

    // Popups (drawn last so they're on top).
    popup::render(frame, app, editor_rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn renders_empty_with_statusline() {
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = App::new();
        term.draw(|f| render(f, &mut app)).unwrap();
        // Last row should contain the NORMAL mode badge.
        let buf = term.backend().buffer().clone();
        let mut bottom = String::new();
        for x in 0..buf.area.width {
            bottom.push_str(buf[(x, buf.area.height - 1)].symbol());
        }
        assert!(
            bottom.contains("NORMAL"),
            "expected NORMAL in statusline, got {bottom:?}"
        );
    }

    #[test]
    fn screen_to_buffer_accounts_for_scroll_and_gutter() {
        use crate::app::{BufferId, ViewId};
        use crate::layout::View;
        let mut v = View::new(ViewId(1), BufferId(0));
        v.scroll = (10, 5);
        let (row, col) = v.screen_to_buffer(2, 8, 4);
        assert_eq!(row, 12);
        assert_eq!(col, 9);
    }
}
