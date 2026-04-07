//! UI state for the completion popup.

use lsp_types::CompletionItem;

#[derive(Default, Debug, Clone)]
pub struct CompletionPopup {
    pub items: Vec<CompletionItem>,
    pub selected: usize,
}

impl CompletionPopup {
    pub fn new(items: Vec<CompletionItem>) -> Self {
        Self { items, selected: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected)
    }

    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
    }

    pub fn prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pop(n: usize) -> CompletionPopup {
        CompletionPopup::new(
            (0..n)
                .map(|i| CompletionItem {
                    label: format!("item{i}"),
                    ..Default::default()
                })
                .collect(),
        )
    }

    #[test]
    fn wrap_around() {
        let mut p = pop(3);
        assert_eq!(p.selected, 0);
        p.next();
        p.next();
        assert_eq!(p.selected, 2);
        p.next();
        assert_eq!(p.selected, 0);
        p.prev();
        assert_eq!(p.selected, 2);
    }

    #[test]
    fn empty_no_panic() {
        let mut p = pop(0);
        p.next();
        p.prev();
        assert!(p.selected_item().is_none());
    }
}
