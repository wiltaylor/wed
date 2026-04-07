use async_trait::async_trait;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::AppEvent;

#[async_trait]
pub trait Pane: Send + Sync {
    fn name(&self) -> &str;
    fn title(&self) -> &str { self.name() }
    fn icon(&self) -> &str { "" }

    fn render(&self, _frame: &mut Frame<'_>, _area: Rect) {}
    fn handle_key(&mut self, _key: KeyEvent) {}
    fn handle_mouse(&mut self, _mouse: MouseEvent) {}
    fn update(&mut self, _event: &AppEvent) {}

    async fn on_event(&mut self) {}
}
