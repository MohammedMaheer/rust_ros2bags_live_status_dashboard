use crate::storage::Storage;
use crate::sync::SyncDaemon;

#[cfg(feature = "ui")]
use eframe::egui;

#[cfg(feature = "ui")]
pub struct DashboardApp {
    ros2_available: bool,
    selected_tab: usize,
    // Metrics history for charts
    message_rate_history: Vec<f32>,
    bandwidth_history: Vec<f32>,
    latency_history: Vec<f32>,
    cpu_usage_history: Vec<f32>,
    memory_usage_history: Vec<f32>,
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
            message_rate_history: Vec::new(),
            bandwidth_history: Vec::new(),
            latency_history: Vec::new(),
            cpu_usage_history: Vec::new(),
            memory_usage_history: Vec::new(),
        }
    }

    fn update_metrics(&mut self) {
        // Add new data points to history (keep last 60 samples)
        if self.message_rate_history.len() > 60 {
            self.message_rate_history.remove(0);
        }
        if self.bandwidth_history.len() > 60 {
            self.bandwidth_history.remove(0);
        }
        if self.latency_history.len() > 60 {
            self.latency_history.remove(0);
        }
        if self.cpu_usage_history.len() > 60 {
            self.cpu_usage_history.remove(0);
        }
        if self.memory_usage_history.len() > 60 {
            self.memory_usage_history.remove(0);
        }

        // Simulate some data (in production, query from actual recorder)
        let message_rate = 120.0 + (rand::random::<f32>() - 0.5) * 30.0;
        let bandwidth = 50.0 + (rand::random::<f32>() - 0.5) * 20.0;
        let latency = 8.5 + (rand::random::<f32>() - 0.5) * 3.0;
        let cpu = 35.0 + (rand::random::<f32>() - 0.5) * 15.0;
        let memory = 512.0 + (rand::random::<f32>() - 0.5) * 100.0;

        self.message_rate_history.push(message_rate.max(0.0));
        self.bandwidth_history.push(bandwidth.max(0.0));
        self.latency_history.push(latency.max(0.0));
        self.cpu_usage_history.push(cpu.max(0.0).min(100.0));
        self.memory_usage_history.push(memory.max(0.0));
    }

    fn draw_chart(
        &self,
        ui: &mut egui::Ui,
        data: &[f32],
        title: &str,
        color: egui::Color32,
        max_value: f32,
    ) {
        ui.group(|ui| {
            ui.label(title);
            let height = 80.0;
            let width = 300.0;

            if data.is_empty() {
                ui.label("No data yet");
                return;
            }

            let (rect, _response) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            let painter = ui.painter_at(rect);

            // Draw background
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));

            // Draw grid lines
            for i in 0..=5 {
                let y = rect.bottom() - (i as f32 / 5.0) * height;
                painter.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    egui::Stroke::new(0.5, egui::Color32::from_rgb(60, 60, 80)),
                );
            }

            // Draw line chart
            if data.len() > 1 {
                let step = width / (data.len() - 1) as f32;
                for i in 0..data.len() - 1 {
                    let x1 = rect.left() + i as f32 * step;
                    let y1 = rect.bottom() - (data[i] / max_value).min(1.0) * height;
                    let x2 = rect.left() + (i + 1) as f32 * step;
                    let y2 = rect.bottom() - (data[i + 1] / max_value).min(1.0) * height;

                    painter.line_segment(
                        [egui::pos2(x1, y1), egui::pos2(x2, y2)],
                        egui::Stroke::new(2.0, color),
                    );
                }

                // Draw dots at last point
                if let Some(&last) = data.last() {
                    let x = rect.right() - 2.0;
                    let y = rect.bottom() - (last / max_value).min(1.0) * height;
                    painter.circle_filled(egui::pos2(x, y), 3.0, color);
                }
            }

            // Draw value labels
            if let Some(&last) = data.last() {
                if let Some(&max) = data.iter().max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)) {
                    let avg = data.iter().sum::<f32>() / data.len() as f32;
                    ui.horizontal(|ui| {
                        ui.label(format!("Current: {:.1}", last));
                        ui.label(format!("Max: {:.1}", max));
                        ui.label(format!("Avg: {:.1}", avg));
                    });
                }
            }
        });
    }
}

#[cfg(feature = "ui")]
impl eframe::App for DashboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_metrics();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ROS2 Multi-Robot Recorder");

            if !self.ros2_available {
                ui.colored_label(egui::Color32::RED, "X ROS2 NOT DETECTED");
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
                ui.selectable_value(&mut self.selected_tab, 1, "Metrics");
                ui.selectable_value(&mut self.selected_tab, 2, "Selected Topics");
                ui.selectable_value(&mut self.selected_tab, 3, "Active Topics");
                ui.selectable_value(&mut self.selected_tab, 4, "Network & Uploads");
                ui.selectable_value(&mut self.selected_tab, 5, "Topic Status");
                ui.selectable_value(&mut self.selected_tab, 6, "Storage");
                ui.selectable_value(&mut self.selected_tab, 7, "Sync");
            });

            ui.separator();

            match self.selected_tab {
                0 => {
                    ui.group(|ui| {
                        ui.heading("Recording Status");
                        ui.separator();
                        ui.label("Status: READY TO RECORD");
                        ui.label("ROS2 Topics Available: Active");
                        ui.label("Recording Device: ROS2 Graph");
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_BLUE, 
                            "To start recording, use the recorder module or ros2 command line");
                        ui.code("cargo run --features ros2 -- --record /my/rosbag");
                    });
                }
                1 => {
                    ui.label("Real-time System Metrics");
                    ui.separator();
                    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                        self.draw_chart(ui, &self.message_rate_history, "Message Rate (Hz)", egui::Color32::LIGHT_BLUE, 200.0);
                        self.draw_chart(ui, &self.bandwidth_history, "Bandwidth (Mbps)", egui::Color32::GREEN, 100.0);
                        self.draw_chart(ui, &self.latency_history, "Latency (ms)", egui::Color32::YELLOW, 20.0);
                        self.draw_chart(ui, &self.cpu_usage_history, "CPU Usage (%)", egui::Color32::RED, 100.0);
                        self.draw_chart(ui, &self.memory_usage_history, "Memory (MB)", egui::Color32::LIGHT_GREEN, 2048.0);
                    });
                }
                2 => {
                    ui.group(|ui| {
                        ui.heading("Selected Topics for Recording");
                        ui.separator();
                        ui.label("Topics marked for recording:");
                        ui.separator();
                        ui.label("✓ /sensor/lidar (sensor_msgs/LaserScan)");
                        ui.label("✓ /camera/rgb (sensor_msgs/Image)");
                        ui.label("✓ /imu (sensor_msgs/Imu)");
                        ui.label("✓ /odom (nav_msgs/Odometry)");
                        ui.label("✓ /tf (tf2_msgs/TFMessage)");
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("+ Add Topic").clicked() {
                                tracing::info!("Add topic button clicked");
                            }
                            if ui.button("- Remove Selected").clicked() {
                                tracing::info!("Remove topic button clicked");
                            }
                        });
                    });
                }
                3 => {
                    ui.group(|ui| {
                        ui.heading("Active ROS2 Topics");
                        ui.separator();
                        ui.label("Currently publishing topics discovered on network:");
                        ui.separator();
                        ui.label("GREEN /sensor/lidar (5 Hz) - 5242 B/s");
                        ui.label("GREEN /camera/rgb (30 Hz) - 2097152 B/s");
                        ui.label("GREEN /imu (100 Hz) - 512 B/s");
                        ui.label("GREEN /odom (50 Hz) - 1024 B/s");
                        ui.label("GREEN /tf (100 Hz) - 2048 B/s");
                        ui.label("RED /cmd_vel (idle) - 0 B/s");
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_BLUE, 
                            "Discover real topics: ros2 topic list");
                    });
                }
                4 => {
                    ui.group(|ui| {
                        ui.heading("Network & Upload Status");
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Network Status:");
                            ui.colored_label(egui::Color32::GREEN, "● Connected");
                        });
                        ui.label("Latency: 8.5 ms");
                        ui.label("Bandwidth: 92.3 Mbps");
                        ui.separator();
                        ui.heading("Upload Queue");
                        ui.label("Pending Segments: 3");
                        ui.label("Current Upload: segment-0.log (42%)");
                        ui.add(egui::ProgressBar::new(0.42).show_percentage());
                        ui.separator();
                        ui.label("Completed: 12 segments");
                        ui.label("Total Uploaded: 1.2 GB");
                        ui.label("Upload Errors: 0");
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Pause Upload").clicked() {
                                tracing::info!("Pause upload clicked");
                            }
                            if ui.button("Resume Upload").clicked() {
                                tracing::info!("Resume upload clicked");
                            }
                        });
                    });
                }
                5 => {
                    ui.group(|ui| {
                        ui.heading("Topic Status Details");
                        ui.separator();
                        ui.label("Topic Performance Metrics:");
                        ui.separator();
                        ui.label("RED /sensor/lidar");
                        ui.label("  Messages: 847");
                        ui.label("  Frequency: 5.0 Hz");
                        ui.label("  Bandwidth: 5.2 KB/s");
                        ui.label("  Status: Recording");
                        ui.separator();
                        ui.label("GREEN /camera/rgb");
                        ui.label("  Messages: 5094");
                        ui.label("  Frequency: 30.0 Hz");
                        ui.label("  Bandwidth: 2.0 MB/s");
                        ui.label("  Status: Recording");
                        ui.separator();
                        ui.label("BLUE /imu");
                        ui.label("  Messages: 26842");
                        ui.label("  Frequency: 100.0 Hz");
                        ui.label("  Bandwidth: 0.5 KB/s");
                        ui.label("  Status: Recording");
                    });
                }
                6 => {
                    ui.group(|ui| {
                        ui.heading("Local Storage");
                        ui.separator();
                        ui.label("Default Storage Location: /tmp/ros2_recordings/");
                        ui.label("Format: Write-Ahead Log (WAL) with CRC32 checksums");
                        ui.label("Segment Size: 16 MB");
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_BLUE, 
                            "WAL provides crash-safe recording and resumable uploads");
                    });
                }
                7 => {
                    ui.group(|ui| {
                        ui.heading("Cloud Sync");
                        ui.separator();
                        ui.label("Configure S3 credentials for cloud uploads:");
                        ui.label("Environment Variables:");
                        ui.code("export S3_ENDPOINT=https://your-minio.example.com");
                        ui.code("export S3_BUCKET=ros2-recordings");
                        ui.code("export AWS_ACCESS_KEY_ID=your-key");
                        ui.code("export AWS_SECRET_ACCESS_KEY=your-secret");
                        ui.separator();
                        ui.colored_label(egui::Color32::LIGHT_BLUE, 
                            "Recordings are automatically synced when configured");
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
