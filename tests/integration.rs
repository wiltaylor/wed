//! End-to-end integration tests for wed.

use ratatui::layout::Rect;
use wed::config::Config;
use wed::editor::buffer::Buffer;
use wed::highlight::HighlightEngine;
use wed::layout::split::SplitNode;
use wed::layout::view::View;

#[test]
fn buffer_basic_edit_undo() {
    let mut b = Buffer::from_str("hello\nworld\n");
    assert_eq!(b.line_count(), 3);
    b.insert(5, "!");
    assert!(b.dirty);
    let s = b.rope.to_string();
    assert!(s.starts_with("hello!"));
    b.undo();
    assert_eq!(b.rope.to_string(), "hello\nworld\n");
}

#[test]
fn highlight_engine_runs_on_rust_buffer() {
    let mut engine = HighlightEngine::default();
    let b = Buffer::from_str("fn main() { let x = 1; }");
    let _spans = engine.highlight(&b);
}

#[test]
fn config_default_round_trips_through_toml() {
    let cfg = Config::default();
    let s = toml::to_string(&cfg).expect("serialize default config");
    let cfg2: Config = toml::from_str(&s).expect("parse default config");
    assert_eq!(cfg.leader, cfg2.leader);
}

#[test]
fn config_from_user_toml() {
    let toml_src = r#"
leader = " "
[editor]
tab_width = 2
[keybindings.normal]
"j" = "cursor.move_down"
"#;
    let cfg = Config::from_str(toml_src).expect("parse user toml");
    assert_eq!(cfg.editor.tab_width, 2);
}

#[test]
fn split_tree_round_trip() {
    let mut root = SplitNode::Leaf(View::default());
    let active = match &root {
        SplitNode::Leaf(v) => v.id,
        _ => unreachable!(),
    };
    let new_id = wed::app::ViewId(999);
    root.split_active(active, wed::layout::split::Direction::Right, new_id);
    let rects = root.layout_rects(Rect::new(0, 0, 80, 24));
    assert!(rects.len() >= 2);
    root.close_active(active);
    let rects = root.layout_rects(Rect::new(0, 0, 80, 24));
    assert_eq!(rects.len(), 1);
}

#[tokio::test]
async fn lsp_protocol_round_trip() {
    use tokio::io::duplex;
    use wed::lsp::protocol::{read_message, write_message};
    let (a, b) = duplex(4096);
    let (_ar, mut aw) = tokio::io::split(a);
    let (mut br, _bw) = tokio::io::split(b);
    let msg = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"ping"});
    let m2 = msg.clone();
    let writer = tokio::spawn(async move {
        write_message(&mut aw, &m2).await.unwrap();
    });
    let mut buf = tokio::io::BufReader::new(&mut br);
    let received: serde_json::Value = read_message(&mut buf).await.unwrap();
    writer.await.unwrap();
    assert_eq!(received["method"], "ping");
}

#[test]
fn dap_breakpoint_store_persists() {
    use wed::dap::breakpoints::BreakpointStore;
    let dir = tempfile::tempdir().unwrap();
    let mut store = BreakpointStore::default();
    let f = std::path::PathBuf::from("foo.rs");
    store.toggle(&f, 10);
    store.save(dir.path()).unwrap();
    let loaded = BreakpointStore::load(dir.path()).unwrap();
    assert_eq!(loaded.get(&f).len(), 1);
}
