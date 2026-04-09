# wed

Wil's Editor — a modal, vim-flavoured terminal editor written in Rust.

> **Status:** early and under active development. Many ex-commands are stubs and
> several niceties (CLI file args, user-config loading) are not yet wired. The
> hotkeys and ex-commands listed below are the ones that actually work today.

---

## Build & run

Cargo:

```sh
cargo build              # debug build
cargo build --release    # release build
cargo run                # run debug build
```

Just recipes (see `justfile`):

```sh
just build
just run
just test
just lint                # cargo clippy -D warnings
just fmt
just ci                  # fmt-check + lint + test
```

### Environment

- `RUST_LOG` controls log verbosity (default `info`). Standard
  `tracing-subscriber` filter syntax, e.g. `RUST_LOG=debug` or
  `RUST_LOG=wed=trace,info`.
- Logs are written to `$XDG_CACHE_HOME/wed/wed.log` (falls back to the system
  temp dir if no cache dir is available). The directory is created on startup.

### Opening files

`wed` does **not** yet parse command-line arguments — `cargo run -- foo.rs`
will start with an empty buffer. Open files from inside the editor with
`:e <path>`.

---

## Configuration

Configuration is TOML. The schema lives in `src/config/schema.rs` and the
loader in `src/config/mod.rs` (`Config::load`).

> **Caveat:** the loader is implemented and tested, but `App::new` does not
> currently call it — wed runs with hard-coded defaults today. The schema below
> is documented so you know what's coming and so config files written now stay
> forward-compatible.

### Top-level sections

| Section | Purpose |
|---|---|
| `leader` | String, leader-key sequence (default `"<space>"`). |
| `[editor]` | Buffer behavior: `tab_width`, `expand_tabs`, `line_numbers`, `relative_line_numbers`, `scroll_off`, `cursor_style`, `auto_indent`, `smart_indent`, `wrap`, `highlight_line`, `color_column`, `show_whitespace`, `undo_limit`. |
| `[ui]` | UI chrome: `tabline`, `statusline`, `left_sidebar_width`, `right_sidebar_width`, `popup_width`, `popup_height`, `icons`. |
| `[search]` | `case_sensitive`, `hidden_files`, `max_results`. |
| `[terminal]` | `shell`. |
| `[theme]` | Color theme. |
| `[keybindings.<mode>]` | Per-mode bindings (`normal`, `insert`, `visual`, `replace`, `command`, `search`). |
| `[leader_bindings]` | Shortcuts under the leader key. |
| `[lsp.<name>]` | LSP server: `command`, `args`, `filetypes`, `root_patterns`. |
| `[dap.<name>]` | Debug adapter: `command`, `args`, `type`, `port_range`, `configurations`. |
| `[filetype.<name>]` | Per-filetype overrides: `extensions`, `language_id`, `tab_width`, `expand_tabs`, `comment`. |

### Example

```toml
leader = "<space>"

[editor]
tab_width = 2
expand_tabs = false

[ui]
tabline = false

[lsp.rust]
command = "rust-analyzer"
filetypes = ["rust"]

[keybindings.normal]
"jk" = "mode.normal"

[leader_bindings]
"ff" = "search.files"
```

### Keybinding syntax

- Literal characters: `"j"`, `"$"`, `"0"`.
- Named keys: `escape`, `enter`, `tab`, `space`, `backspace`, `delete`,
  `insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`,
  `F1`–`F12`.
- Modifiers: `ctrl-x`, `alt-x`, `shift-F11`.
- Chords: `ctrl-w-v` means Ctrl+W then V.
- Leader: `<leader>ff` expands to the leader key followed by `ff`.
- Binding values can be either a bare command name (`"mode.normal"`) or a
  table with args: `{ command = "search.replace", args = ["--flag"] }`.

---

## Hotkeys

wed is modal. The status line shows the current mode. `Esc` always returns to
Normal and clears any pending state.

### Normal mode

**Motions**

| Key | Action |
|---|---|
| `h` `j` `k` `l` (or arrows) | Left / down / up / right |
| `w` / `W` | Next word / WORD start |
| `b` / `B` | Previous word / WORD start |
| `e` / `E` | End of word / WORD |
| `0` | Line start (column 0) |
| `^` | First non-blank on line |
| `$` | Line end |
| `gg` | Buffer start |
| `G` | Buffer end (or `<count>G` to jump to line) |
| `f<c>` / `F<c>` | Find char forward / backward on line |
| `t<c>` / `T<c>` | Till char forward / backward |
| `;` / `,` | Repeat last find / repeat in opposite direction |
| `%` | Jump to matching bracket |
| `(` / `)` | Sentence backward / forward |
| `{` / `}` | Paragraph backward / forward |
| `H` / `M` / `L` | Top / middle / bottom of viewport |
| `Ctrl+d` / `Ctrl+u` | Half-page down / up |
| `Ctrl+f` / `Ctrl+b` | Page down / up |

**Counts.** Any digit prefix multiplies the next motion or operator (e.g.
`3l`, `2dw`). A leading `0` only counts when a count is already in progress —
otherwise it means "line start".

**Operators.** Followed by a motion, a text object, or doubled for linewise:

| Key | Action |
|---|---|
| `d` | Delete |
| `c` | Change (delete + insert) |
| `y` | Yank (copy) |
| `>` / `<` | Indent / dedent |
| `gc` | Toggle comment |

Doubling the operator (`dd`, `cc`, `yy`, `>>`, `<<`) operates on the whole
line.

**Text objects** (used with operators):

| Object | Inner / Around |
|---|---|
| word | `iw` / `aw` |
| `"` `'` quotes | `i"` `a"` / `i'` `a'` |
| `()` | `i(` `i)` / `a(` `a)` |
| `[]` | `i[` `i]` / `a[` `a]` |
| `{}` | `i{` `i}` / `a{` `a}` |
| `<>` | `i<` `i>` / `a<` `a>` |
| paragraph | `ip` / `ap` |

**Edits**

| Key | Action |
|---|---|
| `x` | Delete character under cursor |
| `p` / `P` | Paste after / before cursor |
| `u` | Undo |
| `Ctrl+r` | Redo |
| `.` | Repeat last change |

**Mode switches**

| Key | Action |
|---|---|
| `i` / `I` | Insert at cursor / at first non-blank |
| `a` / `A` | Append after cursor / at line end |
| `o` / `O` | Open new line below / above |
| `v` | Visual (characterwise) |
| `V` | Visual (linewise) |
| `Ctrl+v` | Visual (blockwise) |
| `R` | Replace mode |
| `:` | Command line |
| `/` / `?` | Search forward / backward |

**Search**

| Key | Action |
|---|---|
| `/` `?` | Open search prompt |
| `n` / `N` | Next / previous match |

**Marks**

| Key | Action |
|---|---|
| `m<a-z>` | Set mark |
| `'<a-z>` | Jump to mark |

**Windows** (`Ctrl+W` chords)

| Chord | Action |
|---|---|
| `Ctrl+W v` | Split vertical |
| `Ctrl+W s` | Split horizontal |
| `Ctrl+W h/j/k/l` | Focus left / down / up / right |
| `Ctrl+W q` | Close split |

### Insert mode

| Key | Action |
|---|---|
| `Esc` / `Ctrl+c` | Return to normal |
| `Backspace` | Delete previous char |
| `Enter` | Newline |
| `Tab` | Insert 4 spaces (currently hard-coded) |
| `Ctrl+w` | Delete word backward |
| `Ctrl+u` | Delete to line start |
| any printable | Insert at cursor |

### Visual modes (`v`, `V`, `Ctrl+v`)

- All Normal-mode motions extend the selection.
- `d`, `c`, `y`, `>`, `<` operate on the selection and return to Normal (or
  Insert, after `c`).
- `Esc` exits.

### Replace mode (`R`)

- Each printable key overwrites the character at the cursor and advances.
- `Esc` returns to Normal.

### Operator-pending mode

After pressing `d`, `c`, `y`, `>`, or `<`:

- A motion applies the operator to that range.
- The same operator key again (`dd`, `cc`, …) applies it linewise.
- A text object (`iw`, `a"`, …) applies it to that object.
- A count between operator and motion multiplies the motion (`d2w`).
- `Esc` cancels.

### Command and Search line (`:` and `/`)

| Key | Action |
|---|---|
| printable | Type into the prompt |
| `Backspace` | Delete previous char (empty → exit) |
| `Left` / `Right` | Move cursor in the input |
| `Up` / `Down` | History previous / next |
| `Tab` | Complete command name (Command mode only) |
| `Enter` | Execute command / jump to first match |
| `Esc` | Cancel |

### Ex-commands

| Command | Action |
|---|---|
| `:w` / `:write` | Save current buffer |
| `:q` / `:quit` | Quit |
| `:q!` | Force quit |
| `:wq` / `:x` | Save and quit |
| `:qa` / `:qall` | Quit all |
| `:e <path>` / `:edit <path>` | Open file |
| `:b <name>` / `:buffer <name>` | Switch buffer |
| `:split` / `:sp` | Horizontal split |
| `:vsplit` / `:vsp` | Vertical split |
| `:tabnew` | New tab |
| `:tabn` / `:tabnext` | Next tab |
| `:tabp` / `:tabprev` / `:tabprevious` | Previous tab |
| `:close` | Close view |
| `:<N>` | Jump to line `N` |
| `:[range]s/pat/repl/flags` | Substitute. Ranges: `%` (whole file), `.` (current line), `.,+N` (N lines from current) |

### Default leader bindings

The default leader key is `Space`.

| Sequence | Action |
|---|---|
| `<leader>ff` | File picker |
| `<leader>fg` | Project grep |
| `<leader>fb` | Buffer list |
| `<leader>w` | Save |
| `<leader>q` | Quit |
| `<leader>e` | Open config |

---

## Contributing

Pull requests are **disabled**. wed is a personal tool I build for my own use,
and I'm not looking to take on the maintenance burden of merging and supporting
outside contributions. The source is open so others can read it, learn from it,
and use it as inspiration to build their own editor — fork freely.
