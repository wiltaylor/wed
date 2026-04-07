use crate::app::ServerId;

#[derive(Debug, Default)]
pub struct LspClient {
    pub id: ServerId,
    pub name: String,
}
