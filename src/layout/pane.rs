use async_trait::async_trait;

#[async_trait]
pub trait Pane: Send + Sync {
    fn name(&self) -> &str;
    async fn on_event(&mut self) {}
}
