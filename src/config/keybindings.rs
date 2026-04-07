use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeybindingConfig {
    pub normal: HashMap<String, String>,
    pub insert: HashMap<String, String>,
    pub visual: HashMap<String, String>,
    pub command: HashMap<String, String>,
}
