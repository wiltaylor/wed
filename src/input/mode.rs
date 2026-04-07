//! Editor modes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualKind {
    Char,
    Line,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
    Indent,
    Dedent,
    Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKey {
    /// `g` prefix awaiting follow-up (e.g. `gg`, `gc`).
    G,
    /// Awaiting char for `f`/`F`/`t`/`T`.
    FindChar { forward: bool, till: bool },
    /// `m` awaiting register letter.
    SetMark,
    /// `'` awaiting register letter.
    JumpMark,
    /// `r` awaiting replacement char.
    Replace,
    /// `"` awaiting register name.
    Register,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum EditorMode {
    #[default]
    Normal,
    Insert,
    Visual(VisualKind),
    Replace,
    Command,
    Search,
    Pending(PendingKey),
    Operator(Operator),
}

