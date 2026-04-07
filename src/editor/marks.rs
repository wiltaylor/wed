//! Per-buffer marks (`m{a-z}` / `'{a-z}`).

use crate::editor::Cursor;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Marks {
    map: HashMap<char, Cursor>,
}

impl Marks {
    pub fn new() -> Self { Self::default() }

    pub fn set(&mut self, name: char, cursor: Cursor) {
        self.map.insert(name, cursor);
    }

    pub fn get(&self, name: char) -> Option<Cursor> {
        self.map.get(&name).copied()
    }

    pub fn clear(&mut self) { self.map.clear(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_mark() {
        let mut m = Marks::new();
        m.set('a', Cursor::new(3, 4));
        assert_eq!(m.get('a').unwrap().row, 3);
    }
}
