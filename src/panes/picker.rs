//! Generic fuzzy picker built on top of `nucleo::Matcher`.
//!
//! A `Picker<T>` owns a list of items and a current query. After updating the
//! query (or items) the caller should invoke [`Picker::refresh`] which uses
//! `nucleo::Matcher` to compute and rank matches.

use nucleo::{Config, Matcher, Utf32String};
use std::path::{Path, PathBuf};

/// Trait used to extract the searchable label from a picker item.
pub trait PickerItem {
    fn label(&self) -> String;
}

impl PickerItem for String {
    fn label(&self) -> String {
        self.clone()
    }
}

impl PickerItem for PathBuf {
    fn label(&self) -> String {
        self.to_string_lossy().into_owned()
    }
}

/// A generic fuzzy picker.
pub struct Picker<T: PickerItem> {
    pub items: Vec<T>,
    pub query: String,
    /// Indices into `items` paired with their match score, sorted descending.
    pub matches: Vec<(usize, i64)>,
    pub selected: usize,
    matcher: Matcher,
}

impl<T: PickerItem> Picker<T> {
    pub fn new(items: Vec<T>) -> Self {
        let mut p = Self {
            items,
            query: String::new(),
            matches: Vec::new(),
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT),
        };
        p.refresh();
        p
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.refresh();
    }

    pub fn refresh(&mut self) {
        self.matches.clear();
        let needle = Utf32String::from(self.query.as_str());
        if self.query.is_empty() {
            for i in 0..self.items.len() {
                self.matches.push((i, 0));
            }
        } else {
            for (idx, item) in self.items.iter().enumerate() {
                let hay = Utf32String::from(item.label().as_str());
                if let Some(score) = self.matcher.fuzzy_match(hay.slice(..), needle.slice(..)) {
                    self.matches.push((idx, score as i64));
                }
            }
            self.matches.sort_by(|a, b| b.1.cmp(&a.1));
        }
        if self.selected >= self.matches.len() {
            self.selected = self.matches.len().saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.matches.len() {
            self.selected += 1;
        }
    }

    pub fn current(&self) -> Option<&T> {
        self.matches
            .get(self.selected)
            .and_then(|(i, _)| self.items.get(*i))
    }
}

// ---------- concrete pickers ----------

/// Recursively walk `root` (respecting .gitignore) and build a file picker.
pub fn picker_files(root: &Path) -> Picker<PathBuf> {
    let mut items = Vec::new();
    for entry in ignore::WalkBuilder::new(root).build().flatten() {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            items.push(entry.path().to_path_buf());
        }
    }
    Picker::new(items)
}

/// Build a buffer picker over a list of buffer display names / paths.
pub fn picker_buffers(buffers: Vec<String>) -> Picker<String> {
    Picker::new(buffers)
}

/// Build a command picker from a `CommandRegistry`.
pub fn picker_commands(registry: &crate::commands::CommandRegistry) -> Picker<String> {
    let mut names: Vec<String> = registry.names().cloned().collect();
    names.sort();
    Picker::new(names)
}

/// Build a picker over files tracked by git in `root`. Falls back to all files.
pub fn picker_git_files(root: &Path) -> Picker<PathBuf> {
    if let Ok(repo) = git2::Repository::open(root) {
        let mut items = Vec::new();
        if let Ok(index) = repo.index() {
            for entry in index.iter() {
                if let Ok(s) = std::str::from_utf8(&entry.path) {
                    items.push(root.join(s));
                }
            }
        }
        Picker::new(items)
    } else {
        picker_files(root)
    }
}

/// Build a picker over diagnostic messages.
pub fn picker_diagnostics(diags: Vec<String>) -> Picker<String> {
    Picker::new(diags)
}

/// Build a picker over a flat list of LSP-style symbols (label-only).
pub fn picker_symbols(symbols: Vec<String>) -> Picker<String> {
    Picker::new(symbols)
}

// ---------- just recipes ----------

/// A just recipe with its namepath (for execution) and display label.
pub struct JustRecipe {
    /// The namepath used to run the recipe, e.g. `"mod::build"`.
    pub namepath: String,
    /// Display label: `"namepath — description"` or just `"namepath"`.
    display: String,
}

impl PickerItem for JustRecipe {
    fn label(&self) -> String {
        self.display.clone()
    }
}

/// Build a picker over public `just` recipes (including submodules).
/// Returns `None` if `just` is not available or no justfile is found.
pub fn picker_just_recipes() -> Option<Picker<JustRecipe>> {
    let output = std::process::Command::new("just")
        .args(["--dump", "--dump-format", "json", "--list-submodules"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let mut items = Vec::new();
    collect_recipes(&json, &mut items);
    if items.is_empty() {
        return None;
    }
    Some(Picker::new(items))
}

fn collect_recipes(node: &serde_json::Value, out: &mut Vec<JustRecipe>) {
    if let Some(recipes) = node.get("recipes").and_then(|r| r.as_object()) {
        for recipe in recipes.values() {
            let private = recipe.get("private").and_then(|v| v.as_bool()).unwrap_or(false);
            if private {
                continue;
            }
            let namepath = recipe
                .get("namepath")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let doc = recipe.get("doc").and_then(|v| v.as_str()).unwrap_or_default();
            let display = if doc.is_empty() {
                namepath.clone()
            } else {
                format!("{namepath} — {doc}")
            };
            out.push(JustRecipe { namepath, display });
        }
    }
    if let Some(modules) = node.get("modules").and_then(|m| m.as_object()) {
        for module in modules.values() {
            collect_recipes(module, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_ab_before_axb() {
        let mut p = Picker::new(vec!["abc".to_string(), "axb".to_string()]);
        p.set_query("ab");
        assert!(!p.matches.is_empty());
        let first = p.matches[0].0;
        assert_eq!(p.items[first], "abc");
    }
}
