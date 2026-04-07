# Terminal Editor — Architecture Specification

> A Rust-native, async, modal terminal editor with vim keybindings, LSP/DAP, tree-sitter, sidebar panes, and WCL configuration. No plugin system — all features built-in.

---

## Working Name

**`wed`** *(working title — swap freely)*

---

## Guiding Principles

- **All actions are commands.** Every editor operation has a named command (e.g. `editor.save`, `buffer.split.vertical`). Keys, mouse events, and the `:` terminal all invoke the same command registry.
- **Async-first.** LSP, DAP, file I/O, and git ops are all non-blocking via Tokio. The UI thread never blocks.
- **No plugin system.** Feature set is fixed and curated. All pane types, LSP/DAP support, etc. are first-class.
- **WCL for configuration.** Editor config, keybindings, LSP server definitions, DAP adapters, themes, and leader mappings are all declared in `.wcl` files.

---

## Crate Selection

| Concern | Crate | Notes |
|---|---|---|
| Async runtime | `tokio` (full features) | Task-per-LSP/DAP server |
| TUI rendering | `ratatui` | Crossterm backend |
| Terminal backend | `crossterm` | Windows + Linux cross-platform |
| Text storage | `ropey` | Rope data structure — O(log n) edits |
| Syntax highlighting | `tree-sitter` + grammars | See grammar list below |
| LSP client | `lsp-types` + custom client | Tower-based async channels |
| DAP client | `dap` crate or custom | Async DAP session manager |
| Git integration | `git2` | libgit2 bindings for git pane |
| WCL config | `wcl` (your crate) | Config loading + schema validation |
| Fuzzy find | `nucleo` | Telescope-style file/symbol picker |
| File watching | `notify` | Config reload, file change detection |
| Unicode | `unicode-width` + `unicode-segmentation` | Correct cursor positioning |
| Clipboard | `arboard` | Cross-platform clipboard |
| Serialization | `serde` + `serde_json` | LSP/DAP message encoding |
| Error handling | `anyhow` + `thiserror` | Editor errors + LSP/DAP protocol errors |
| Logging | `tracing` + `tracing-subscriber` | Log to file (not stdout — TUI owns it) |
| Async channels | `tokio::sync::mpsc` + `broadcast` | Event bus between components |
| Process management | `tokio::process` | Spawn LSP/DAP server processes |
| Embedded terminal | `portable-pty` | ConPTY on Windows, PTY on Linux |

See the user-supplied specification message for the full text including module tree, data model, event system, modes, command system, keybindings, command line, LSP/DAP details, WCL schema, mouse table, theme, justfile, git conventions, and milestones M1–M8. This file mirrors that specification verbatim and is the canonical reference for implementation work in this repository.

---

## Milestones

| Milestone | Features |
|---|---|
| M1 — Shell | Ratatui loop, crossterm input, buffer + rope, normal/insert mode basics, no highlighting |
| M2 — Editor Core | Full vim motions, visual mode, undo/redo, splits, tabs, status line, command line |
| M3 — Highlighting | Tree-sitter integration, grammar registry, theme loading from WCL |
| M4 — LSP | LSP client, diagnostics, hover, completion popup, go-to-definition |
| M5 — Sidebar | File browser pane, git pane, diagnostics pane, sidebar layout |
| M6 — Mouse | Full mouse support: click-to-move, drag selection, scroll, split resize |
| M7 — DAP | Debug session, breakpoints, step/continue, inline variables, DAP panes |
| M8 — Polish | Fuzzy picker, project search, leader bindings, all WCL config, Windows parity |
