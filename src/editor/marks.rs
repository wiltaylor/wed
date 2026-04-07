use crate::editor::Cursor;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Marks {
    pub map: HashMap<char, Cursor>,
}
