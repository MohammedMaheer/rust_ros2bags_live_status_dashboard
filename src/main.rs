use anyhow::Result;
use tracing::info;

mod config;
mod dashboard;
mod diagnostics;
mod exporter;
mod recorder;
mod storage;
mod sync;
mod network;
mod utils;

use config::AppConfig;
use sync::SyncDaemon;
use diagnostics::detect_ros2_available;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting rust_ros2_recorder");

    let config = AppConfig::load_default()?;

    // Initialize storage and WAL
    let storage = storage::Storage::new(&config.storage).await?;

    // Start background sync daemon
    let sync_daemon = SyncDaemon::new(storage.clone(), config.sync.clone());
    let sync_handle = {
        let daemon = sync_daemon.clone();
        tokio::spawn(async move {
            daemon.sync_loop(7).await;
        })
    };

    // Start recorder (ROS2) - may be stubbed if ROS2 not enabled
    let recorder_handle = recorder::start_recorder(storage.clone(), config.clone());

    // Detect if ROS2 is available
    let ros2_available = detect_ros2_available();

    // Run dashboard UI (blocking on UI thread)
    // When dashboard closes, app exits
    match dashboard::run_dashboard(storage.clone(), sync_daemon.clone(), ros2_available) {
        Ok(_) => info!("Dashboard closed cleanly"),
        Err(e) => eprintln!("Dashboard error: {:#?}", e),
    }

    // Cancel background tasks
    sync_handle.abort();
    recorder_handle.abort();

    Ok(())
}
