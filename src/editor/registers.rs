//! Vim-style registers with optional system clipboard fallback.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum YankKind {
    #[default]
    Char,
    Line,
    Block,
}


#[derive(Debug, Clone, Default)]
pub struct RegisterEntry {
    pub text: String,
    pub kind: YankKind,
}

#[derive(Debug, Default)]
pub struct Registers {
    map: HashMap<char, RegisterEntry>,
}

impl Registers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: char, entry: RegisterEntry) {
        // Always mirror into unnamed register `"`.
        self.map.insert('"', entry.clone());
        if name != '"' {
            self.map.insert(name, entry);
        }
    }

    pub fn set_unnamed(&mut self, text: impl Into<String>, kind: YankKind) {
        self.set(
            '"',
            RegisterEntry {
                text: text.into(),
                kind,
            },
        );
    }

    pub fn get(&self, name: char) -> Option<&RegisterEntry> {
        self.map.get(&name)
    }

    /// Try to read system clipboard via arboard; falls back to unnamed register.
    pub fn system_clipboard_get(&self) -> Option<RegisterEntry> {
        #[cfg(any())]
        {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Ok(text) = cb.get_text() {
                    return Some(RegisterEntry {
                        text,
                        kind: YankKind::Char,
                    });
                }
            }
        }
        self.get('"').cloned()
    }

    pub fn system_clipboard_set(&mut self, entry: RegisterEntry) {
        #[cfg(any())]
        {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(entry.text.clone());
            }
        }
        self.set('+', entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unnamed_mirrors_named() {
        let mut r = Registers::new();
        r.set(
            'a',
            RegisterEntry {
                text: "hi".into(),
                kind: YankKind::Char,
            },
        );
        assert_eq!(r.get('a').unwrap().text, "hi");
        assert_eq!(r.get('"').unwrap().text, "hi");
    }
}
