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
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
            .with_writer(file)
            .try_init();
    }

    let mut app = App::new();
    if let Err(e) = app.run().await {
        eprintln!("wed: error: {e:#}");
        return Err(e);
    }
    Ok(())
}
