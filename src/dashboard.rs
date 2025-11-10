use crate::storage::Storage;
use crate::sync::SyncDaemon;

#[cfg(feature = "ui")]
use eframe::egui;

#[cfg(feature = "ui")]
pub struct DashboardApp {
    ros2_available: bool,
    selected_tab: usize,
}

#[cfg(feature = "ui")]
pub fn run_dashboard(
    _storage: Storage,
    _sync_daemon: SyncDaemon,
    ros2_available: bool,
) -> anyhow::Result<()> {
    if !ros2_available {
        // Don't show UI if ROS2 not available - this is ROS2-only
        tracing::warn!("ROS2 not detected. This is a ROS2-only recorder.");
        tracing::warn!("Set ROS_DISTRO or ROS_DOMAIN_ID environment variables and restart.");
        return Ok(());
    }

    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "ROS2 Recording Dashboard",
        options,
        Box::new(move |_cc| Box::new(DashboardApp::new(ros2_available))),
    );
    Ok(())
}

#[cfg(feature = "ui")]
impl DashboardApp {
    fn new(ros2_available: bool) -> Self {
        Self {
            ros2_available,
            selected_tab: 0,
        }
    }
}

#[cfg(feature = "ui")]
impl eframe::App for DashboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🤖 ROS2 Multi-Robot Recorder");

            if !self.ros2_available {
                ui.colored_label(egui::Color32::RED, "✗ ROS2 NOT DETECTED");
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, "This is a ROS2-ONLY recorder.");
                ui.separator();
                ui.label("Setup Instructions:");
                ui.code("export ROS_DISTRO=humble");
                ui.code("export ROS_DOMAIN_ID=0");
                ui.label("Then restart this application");
                ui.separator();
                ui.label("Verify ROS2 installation:");
                ui.code("ros2 topic list");
                return;
            }

            ui.colored_label(egui::Color32::GREEN, "✓ ROS2 DETECTED - LIVE MODE");
            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.selected_tab, 0, "Overview");
                ui.selectable_value(&mut self.selected_tab, 1, "Topics");
                ui.selectable_value(&mut self.selected_tab, 2, "Storage");
                ui.selectable_value(&mut self.selected_tab, 3, "Sync");
            });

            ui.separator();

            match self.selected_tab {
                0 => {
                    ui.group(|ui| {
                        ui.heading("📊 Recording Status");
                        ui.separator();
                        ui.label("Status: READY TO RECORD");
                        ui.label("ROS2 Topics Available: Active");
                        ui.label("Recording Device: ROS2 Graph");
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_BLUE, 
                            "💡 To start recording, use the recorder module or ros2 command line");
                        ui.code("cargo run --features ros2 -- --record /my/rosbag");
                    });
                }
                1 => {
                    ui.group(|ui| {
                        ui.heading("� Available ROS2 Topics");
                        ui.separator();
                        ui.label("Use ros2 CLI to discover topics:");
                        ui.code("ros2 topic list");
                        ui.code("ros2 topic info /topic_name");
                        ui.separator();
                        ui.label("The recorder will automatically discover and record all topics");
                        ui.label("published in your ROS2 domain.");
                    });
                }
                2 => {
                    ui.group(|ui| {
                        ui.heading("� Local Storage");
                        ui.separator();
                        ui.label("Default Storage Location: /tmp/ros2_recordings/");
                        ui.label("Format: Write-Ahead Log (WAL) with CRC32 checksums");
                        ui.label("Segment Size: 16 MB");
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_BLUE, 
                            "💡 WAL provides crash-safe recording and resumable uploads");
                    });
                }
                3 => {
                    ui.group(|ui| {
                        ui.heading("☁️ Cloud Sync");
                        ui.separator();
                        ui.label("Configure S3 credentials for cloud uploads:");
                        ui.label("Environment Variables:");
                        ui.code("export S3_ENDPOINT=https://your-minio.example.com");
                        ui.code("export S3_BUCKET=ros2-recordings");
                        ui.code("export AWS_ACCESS_KEY_ID=your-key");
                        ui.code("export AWS_SECRET_ACCESS_KEY=your-secret");
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_BLUE, 
                            "💡 Recordings are automatically synced when configured");
                    });
                }
                _ => {}
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}

#[cfg(not(feature = "ui"))]
pub fn run_dashboard(
    _storage: Storage,
    _sync_daemon: SyncDaemon,
    _ros2_available: bool,
) -> anyhow::Result<()> {
    tracing::info!("Dashboard requires 'ui' feature. Build with: cargo build --features ui");
    Ok(())
}
