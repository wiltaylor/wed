use ratatui::layout::Rect;

use crate::app::ViewId;
use crate::layout::View;

/// Direction for splitting and focus movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug)]
pub enum SplitNode {
    Leaf(View),
    /// Side-by-side split. `ratio` is the fraction of width given to `left`.
    Horizontal { ratio: f32, left: Box<SplitNode>, right: Box<SplitNode> },
    /// Stacked split. `ratio` is the fraction of height given to `top`.
    Vertical { ratio: f32, top: Box<SplitNode>, bottom: Box<SplitNode> },
}

impl Default for SplitNode {
    fn default() -> Self {
        SplitNode::Leaf(View::default())
    }
}

impl SplitNode {
    /// Split the currently active leaf in the given direction. The active view is
    /// duplicated into the new pane and `new_id` becomes the id of the freshly
    /// created sibling. Returns the id of the new view if a split happened.
    pub fn split_active(&mut self, active: ViewId, direction: Direction, new_id: ViewId) -> Option<ViewId> {
        // find leaf and replace in place
        let path = self.path_to(active)?;
        let leaf = self.get_mut_by_path(&path);
        let old = std::mem::take(leaf);
        let (orig_view, mut new_view) = match old {
            SplitNode::Leaf(v) => {
                let mut nv = v.clone();
                nv.id = new_id;
                (SplitNode::Leaf(v), nv)
            }
            other => {
                *leaf = other;
                return None;
            }
        };
        new_view.cursor = (0, 0);
        new_view.scroll = (0, 0);
        let new_leaf = SplitNode::Leaf(new_view);
        *leaf = match direction {
            Direction::Right => SplitNode::Horizontal {
                ratio: 0.5,
                left: Box::new(orig_view),
                right: Box::new(new_leaf),
            },
            Direction::Left => SplitNode::Horizontal {
                ratio: 0.5,
                left: Box::new(new_leaf),
                right: Box::new(orig_view),
            },
            Direction::Down => SplitNode::Vertical {
                ratio: 0.5,
                top: Box::new(orig_view),
                bottom: Box::new(new_leaf),
            },
            Direction::Up => SplitNode::Vertical {
                ratio: 0.5,
                top: Box::new(new_leaf),
                bottom: Box::new(orig_view),
            },
        };
        Some(new_id)
    }

    /// Close the active leaf, collapsing the parent split. Returns the id of a
    /// new active view (a sibling) if one exists. If the closed leaf was the
    /// only leaf in the tree, the tree is replaced with a default empty leaf
    /// and `None` is returned.
    pub fn close_active(&mut self, active: ViewId) -> Option<ViewId> {
        let path = self.path_to(active)?;
        if path.is_empty() {
            // root leaf - replace with default
            *self = SplitNode::Leaf(View::default());
            return None;
        }
        let parent_path = &path[..path.len() - 1];
        let last = *path.last().unwrap();
        let parent = self.get_mut_by_path(parent_path);
        let replacement = match std::mem::take(parent) {
            SplitNode::Horizontal { left, right, .. } => {
                if last == 0 { *right } else { *left }
            }
            SplitNode::Vertical { top, bottom, .. } => {
                if last == 0 { *bottom } else { *top }
            }
            other => other,
        };
        *parent = replacement;
        // Pick first leaf as new active
        self.iter_leaves().next().map(|(id, _)| id)
    }

    /// Move focus from active leaf in the given direction. Returns the new active id.
    pub fn focus(&self, active: ViewId, direction: Direction, area: Rect) -> Option<ViewId> {
        let rects = self.layout_rects(area);
        let cur = rects.iter().find(|(id, _)| *id == active)?.1;
        let (cx, cy) = (cur.x + cur.width / 2, cur.y + cur.height / 2);
        rects
            .iter()
            .filter(|(id, _)| *id != active)
            .filter(|(_, r)| match direction {
                Direction::Left => r.x + r.width <= cur.x,
                Direction::Right => r.x >= cur.x + cur.width,
                Direction::Up => r.y + r.height <= cur.y,
                Direction::Down => r.y >= cur.y + cur.height,
            })
            .min_by_key(|(_, r)| {
                let rcx = r.x + r.width / 2;
                let rcy = r.y + r.height / 2;
                let dx = (rcx as i32 - cx as i32).abs();
                let dy = (rcy as i32 - cy as i32).abs();
                (dx + dy) as u32
            })
            .map(|(id, _)| *id)
    }

    pub fn focus_left(&self, active: ViewId, area: Rect) -> Option<ViewId> {
        self.focus(active, Direction::Left, area)
    }
    pub fn focus_right(&self, active: ViewId, area: Rect) -> Option<ViewId> {
        self.focus(active, Direction::Right, area)
    }
    pub fn focus_up(&self, active: ViewId, area: Rect) -> Option<ViewId> {
        self.focus(active, Direction::Up, area)
    }
    pub fn focus_down(&self, active: ViewId, area: Rect) -> Option<ViewId> {
        self.focus(active, Direction::Down, area)
    }

    /// Resize the split that contains `active` along its axis by `delta` (in ratio units).
    pub fn resize(&mut self, active: ViewId, delta: f32) -> bool {
        let Some(path) = self.path_to(active) else { return false };
        if path.is_empty() {
            return false;
        }
        // walk to the deepest ancestor split and adjust its ratio.
        let parent_path = &path[..path.len() - 1];
        let parent = self.get_mut_by_path(parent_path);
        match parent {
            SplitNode::Horizontal { ratio, .. } | SplitNode::Vertical { ratio, .. } => {
                *ratio = (*ratio + delta).clamp(0.05, 0.95);
                true
            }
            _ => false,
        }
    }

    /// Find a leaf View by id.
    pub fn find(&self, view_id: ViewId) -> Option<&View> {
        match self {
            SplitNode::Leaf(v) => (v.id == view_id).then_some(v),
            SplitNode::Horizontal { left, right, .. } => {
                left.find(view_id).or_else(|| right.find(view_id))
            }
            SplitNode::Vertical { top, bottom, .. } => {
                top.find(view_id).or_else(|| bottom.find(view_id))
            }
        }
    }

    pub fn find_mut(&mut self, view_id: ViewId) -> Option<&mut View> {
        match self {
            SplitNode::Leaf(v) => (v.id == view_id).then_some(v),
            SplitNode::Horizontal { left, right, .. } => {
                left.find_mut(view_id).or_else(|| right.find_mut(view_id))
            }
            SplitNode::Vertical { top, bottom, .. } => {
                top.find_mut(view_id).or_else(|| bottom.find_mut(view_id))
            }
        }
    }

    /// Iterator over all leaves with their ids.
    pub fn iter_leaves(&self) -> Box<dyn Iterator<Item = (ViewId, &View)> + '_> {
        match self {
            SplitNode::Leaf(v) => Box::new(std::iter::once((v.id, v))),
            SplitNode::Horizontal { left, right, .. } => {
                Box::new(left.iter_leaves().chain(right.iter_leaves()))
            }
            SplitNode::Vertical { top, bottom, .. } => {
                Box::new(top.iter_leaves().chain(bottom.iter_leaves()))
            }
        }
    }

    /// Compute (ViewId, Rect) for every leaf inside `area`, recursively splitting per ratio.
    pub fn layout_rects(&self, area: Rect) -> Vec<(ViewId, Rect)> {
        let mut out = Vec::new();
        self.layout_into(area, &mut out);
        out
    }

    fn layout_into(&self, area: Rect, out: &mut Vec<(ViewId, Rect)>) {
        match self {
            SplitNode::Leaf(v) => out.push((v.id, area)),
            SplitNode::Horizontal { ratio, left, right } => {
                let lw = ((area.width as f32) * ratio).round() as u16;
                let lw = lw.min(area.width.saturating_sub(1)).max(1);
                let l = Rect { x: area.x, y: area.y, width: lw, height: area.height };
                let r = Rect {
                    x: area.x + lw,
                    y: area.y,
                    width: area.width - lw,
                    height: area.height,
                };
                left.layout_into(l, out);
                right.layout_into(r, out);
            }
            SplitNode::Vertical { ratio, top, bottom } => {
                let th = ((area.height as f32) * ratio).round() as u16;
                let th = th.min(area.height.saturating_sub(1)).max(1);
                let t = Rect { x: area.x, y: area.y, width: area.width, height: th };
                let b = Rect {
                    x: area.x,
                    y: area.y + th,
                    width: area.width,
                    height: area.height - th,
                };
                top.layout_into(t, out);
                bottom.layout_into(b, out);
            }
        }
    }

    /// Path to the leaf with the given id. Each step is 0 (left/top) or 1 (right/bottom).
    fn path_to(&self, id: ViewId) -> Option<Vec<usize>> {
        match self {
            SplitNode::Leaf(v) => (v.id == id).then(Vec::new),
            SplitNode::Horizontal { left, right, .. } => {
                if let Some(mut p) = left.path_to(id) {
                    p.insert(0, 0);
                    Some(p)
                } else if let Some(mut p) = right.path_to(id) {
                    p.insert(0, 1);
                    Some(p)
                } else {
                    None
                }
            }
            SplitNode::Vertical { top, bottom, .. } => {
                if let Some(mut p) = top.path_to(id) {
                    p.insert(0, 0);
                    Some(p)
                } else if let Some(mut p) = bottom.path_to(id) {
                    p.insert(0, 1);
                    Some(p)
                } else {
                    None
                }
            }
        }
    }

    fn get_mut_by_path(&mut self, path: &[usize]) -> &mut SplitNode {
        let mut node = self;
        for &step in path {
            node = match node {
                SplitNode::Horizontal { left, right, .. } => {
                    if step == 0 { left.as_mut() } else { right.as_mut() }
                }
                SplitNode::Vertical { top, bottom, .. } => {
                    if step == 0 { top.as_mut() } else { bottom.as_mut() }
                }
                SplitNode::Leaf(_) => return node,
            };
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{BufferId, ViewId};

    fn leaf(id: u64) -> SplitNode {
        SplitNode::Leaf(View::new(ViewId(id), BufferId(0)))
    }

    #[test]
    fn split_active_creates_horizontal() {
        let mut root = leaf(1);
        let new = root.split_active(ViewId(1), Direction::Right, ViewId(2));
        assert_eq!(new, Some(ViewId(2)));
        assert_eq!(root.iter_leaves().count(), 2);
    }

    #[test]
    fn close_active_collapses_parent() {
        let mut root = leaf(1);
        root.split_active(ViewId(1), Direction::Right, ViewId(2));
        let next = root.close_active(ViewId(2));
        assert_eq!(next, Some(ViewId(1)));
        assert_eq!(root.iter_leaves().count(), 1);
    }

    #[test]
    fn close_only_leaf_resets() {
        let mut root = leaf(1);
        let next = root.close_active(ViewId(1));
        assert!(next.is_none());
        assert_eq!(root.iter_leaves().count(), 1);
    }

    #[test]
    fn focus_right_picks_neighbor() {
        let mut root = leaf(1);
        root.split_active(ViewId(1), Direction::Right, ViewId(2));
        let area = Rect::new(0, 0, 80, 24);
        assert_eq!(root.focus_right(ViewId(1), area), Some(ViewId(2)));
        assert_eq!(root.focus_left(ViewId(2), area), Some(ViewId(1)));
    }

    #[test]
    fn resize_clamps() {
        let mut root = leaf(1);
        root.split_active(ViewId(1), Direction::Right, ViewId(2));
        assert!(root.resize(ViewId(1), 0.2));
        if let SplitNode::Horizontal { ratio, .. } = &root {
            assert!((ratio - 0.7).abs() < 1e-5);
        } else {
            panic!("expected horizontal");
        }
    }

    #[test]
    fn layout_rects_nested() {
        let mut root = leaf(1);
        root.split_active(ViewId(1), Direction::Right, ViewId(2));
        root.split_active(ViewId(2), Direction::Down, ViewId(3));
        let rects = root.layout_rects(Rect::new(0, 0, 80, 24));
        assert_eq!(rects.len(), 3);
        let total: u32 = rects.iter().map(|(_, r)| r.width as u32 * r.height as u32).sum();
        assert_eq!(total, 80 * 24);
        // No overlap & contiguous coverage check via union area sum equals total
    }
}
