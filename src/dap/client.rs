use crate::app::SessionId;

#[derive(Debug, Default)]
pub struct DapClient {
    pub id: SessionId,
    pub name: String,
}
