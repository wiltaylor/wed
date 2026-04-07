use anyhow::Result;
use tracing_subscriber::EnvFilter;
use wed::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    let log_path = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("wed")
        .join("wed.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
            )
            .with_writer(file)
            .try_init();
    }

    let mut app = App::new();
    for arg in std::env::args().skip(1) {
        if let Err(e) = app.open_file_in_new_tab(std::path::Path::new(&arg)) {
            eprintln!("wed: failed to open {arg}: {e:#}");
        }
    }
    if app.layout.tabs.is_empty() {
        use wed::app::{BufferId, ViewId};
        use wed::layout::{SplitNode, Tab, View};
        let mut scratch = wed::editor::Buffer::default();
        scratch.id = BufferId(0);
        app.buffers.push(scratch);
        let view_id = ViewId(1);
        let view = View::new(view_id, BufferId(0));
        let tab = Tab::new("[scratch]", SplitNode::Leaf(view), view_id);
        app.layout.tabs.push(tab);
    }
    if let Err(e) = app.run().await {
        eprintln!("wed: error: {e:#}");
        return Err(e);
    }
    Ok(())
}
