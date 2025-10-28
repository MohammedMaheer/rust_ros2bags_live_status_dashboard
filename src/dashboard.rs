use crate::diagnostics::MetricsCollector;
use crate::storage::Storage;
use crate::sync::SyncDaemon;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(feature = "ui")]
use eframe::egui;

pub struct DashboardState {
    pub storage: Storage,
    pub sync_daemon: Arc<Mutex<Option<SyncDaemon>>>,
    pub metrics_collector: MetricsCollector,
    pub is_recording: bool,
    pub ros2_available: bool,
}

#[cfg(feature = "ui")]
pub fn run_dashboard(
    _storage: Storage,
    _sync_daemon: SyncDaemon,
    _ros2_available: bool,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "ROS2 Recording Dashboard",
        options,
        Box::new(move |_cc| {
            Box::new(DashboardApp::new(
                _ros2_available,
                _storage.clone(),
                _sync_daemon.clone(),
            ))
        }),
    );
    Ok(())
}

#[cfg(not(feature = "ui"))]
pub async fn run_dashboard(_config: AppConfig, _storage: Storage) -> Result<()> {
    println!("Dashboard: UI feature not enabled");
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

#[cfg(feature = "ui")]
pub struct DashboardApp {
    recording_active: bool,
    mode_demo: bool,
    ros2_available: bool,

    message_rate: f32,
    cpu_usage: f32,
    memory_usage_mb: f32,
    storage_used_mb: f32,
    network_latency_ms: f32,
    upload_speed_mbps: f32,

    message_rate_history: Vec<f32>,
    cpu_history: Vec<f32>,
    memory_history: Vec<f32>,

    sync_status_text: String,
    selected_tab: usize,
    robots: Vec<String>,
    total_messages: u64,
    active_topics: usize,

    #[allow(dead_code)]
    storage: Storage,
    #[allow(dead_code)]
    sync_daemon: SyncDaemon,
}

#[cfg(feature = "ui")]
impl DashboardApp {
    fn new(
        ros2_available: bool,
        storage: Storage,
        sync_daemon: SyncDaemon,
    ) -> Self {
        Self {
            recording_active: false,
            mode_demo: !ros2_available,
            ros2_available,
            message_rate: 0.0,
            cpu_usage: 15.0,
            memory_usage_mb: 256.0,
            storage_used_mb: 0.0,
            network_latency_ms: 5.2,
            upload_speed_mbps: 12.5,
            message_rate_history: Vec::new(),
            cpu_history: Vec::new(),
            memory_history: Vec::new(),
            sync_status_text: if ros2_available { "ROS2 Ready".to_string() } else { "Demo Mode".to_string() },
            selected_tab: 0,
            robots: vec!["robot1".to_string(), "robot2".to_string()],
            total_messages: 0,
            active_topics: 3,
            storage,
            sync_daemon,
        }
    }

    fn update_metrics(&mut self) {
        if self.recording_active {
            self.message_rate = 75.0 + (rand::random::<f32>() - 0.5) * 10.0;
            self.cpu_usage = 35.0 + (rand::random::<f32>() - 0.5) * 15.0;
            self.memory_usage_mb = 350.0 + (rand::random::<f32>() - 0.5) * 50.0;
            self.storage_used_mb += self.message_rate * 0.001;
            self.total_messages += self.message_rate as u64;
        } else {
            self.message_rate = (self.message_rate * 0.95).max(0.0);
            self.cpu_usage = 15.0 + (rand::random::<f32>() - 0.5) * 5.0;
            self.memory_usage_mb = (self.memory_usage_mb * 0.98).max(256.0);
        }

        if self.message_rate_history.len() > 300 {
            self.message_rate_history.remove(0);
            self.cpu_history.remove(0);
            self.memory_history.remove(0);
        }
        self.message_rate_history.push(self.message_rate);
        self.cpu_history.push(self.cpu_usage);
        self.memory_history.push(self.memory_usage_mb / 100.0);
    }

    fn render_sparkline(&self, ui: &mut egui::Ui, values: &[f32], max_value: f32, color: egui::Color32) {
        let height = 40.0;
        let width = 200.0;

        let (rect, _response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

        if values.len() > 1 {
            let step = width / (values.len() - 1) as f32;
            for i in 0..values.len() - 1 {
                let x1 = rect.left() + i as f32 * step;
                let y1 = rect.bottom() - (values[i] / max_value) * height;
                let x2 = rect.left() + (i + 1) as f32 * step;
                let y2 = rect.bottom() - (values[i + 1] / max_value) * height;

                painter.line_segment(
                    [egui::pos2(x1, y1), egui::pos2(x2, y2)],
                    egui::Stroke::new(2.0, color),
                );
            }
        }
    }
}

#[cfg(feature = "ui")]
impl eframe::App for DashboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_metrics();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🤖 ROS2 Multi-Robot Recorder");

            ui.horizontal(|ui| {
                if self.mode_demo {
                    ui.colored_label(egui::Color32::YELLOW, "⚠ DEMO MODE");
                } else {
                    ui.colored_label(egui::Color32::GREEN, "✓ ROS2 MODE");
                }
                if ui.button("Toggle Mode").clicked() {
                    self.mode_demo = !self.mode_demo;
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if self.recording_active {
                    ui.colored_label(egui::Color32::GREEN, "● RECORDING");
                } else {
                    ui.colored_label(egui::Color32::RED, "● IDLE");
                }
                ui.label(format!("Messages: {} | Rate: {:.1} Hz | Storage: {:.1} MB", 
                    self.total_messages, self.message_rate, self.storage_used_mb));
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.selected_tab, 0, "Overview");
                ui.selectable_value(&mut self.selected_tab, 1, "Metrics");
                ui.selectable_value(&mut self.selected_tab, 2, "Topics");
                ui.selectable_value(&mut self.selected_tab, 3, "Sync");
            });

            ui.separator();

            match self.selected_tab {
                0 => {
                    ui.group(|ui| {
                        ui.label("Recording Controls");
                        ui.horizontal(|ui| {
                            if ui.button("▶ Start").clicked() { self.recording_active = true; }
                            if ui.button("⏸ Pause").clicked() { self.recording_active = false; }
                            if ui.button("⏹ Stop").clicked() { self.recording_active = false; }
                            if ui.button("↑ Sync").clicked() { self.sync_status_text = "Syncing...".to_string(); }
                            if ui.button("📊 Export").clicked() { self.sync_status_text = "Exporting...".to_string(); }
                        });
                    });

                    ui.group(|ui| {
                        ui.label("Quick Stats");
                        ui.horizontal(|ui| {
                            ui.label(format!("CPU: {:.1}%", self.cpu_usage));
                            ui.add(egui::ProgressBar::new((self.cpu_usage / 100.0).min(1.0)));
                        });
                        ui.horizontal(|ui| {
                            ui.label(format!("Memory: {:.0} MB", self.memory_usage_mb));
                            ui.add(egui::ProgressBar::new((self.memory_usage_mb / 2000.0).min(1.0)));
                        });
                    });
                }
                1 => {
                    ui.group(|ui| {
                        ui.label("Message Rate (Hz)");
                        ui.label(format!("Current: {:.1} Hz", self.message_rate));
                        if !self.message_rate_history.is_empty() {
                            self.render_sparkline(ui, &self.message_rate_history, 100.0, egui::Color32::LIGHT_BLUE);
                        }
                    });

                    ui.group(|ui| {
                        ui.label("CPU Usage (%)");
                        ui.label(format!("Current: {:.1}%", self.cpu_usage));
                        if !self.cpu_history.is_empty() {
                            self.render_sparkline(ui, &self.cpu_history, 100.0, egui::Color32::LIGHT_GREEN);
                        }
                    });

                    ui.group(|ui| {
                        ui.label("Memory (normalized)");
                        ui.label(format!("Current: {:.0} MB", self.memory_usage_mb));
                        if !self.memory_history.is_empty() {
                            self.render_sparkline(ui, &self.memory_history, 5.0, egui::Color32::LIGHT_YELLOW);
                        }
                    });
                }
                2 => {
                    ui.group(|ui| {
                        ui.label(format!("Active Topics: {}", self.active_topics));
                        ui.separator();
                        ui.label("📊 /sensor/lidar [LaserScan] 50 Hz");
                        ui.label("📊 /tf [TF2] 100 Hz");
                        ui.label("📊 /odometry [Odometry] 25 Hz");
                    });
                }
                3 => {
                    ui.group(|ui| {
                        ui.label("Sync Status");
                        ui.label(format!("Status: {}", self.sync_status_text));
                        ui.label("Queued: 3 segments");
                        ui.label("Errors: 0");
                        ui.add(egui::ProgressBar::new(0.65).show_percentage());
                    });
                }
                _ => {}
            }
        });

        ctx.request_repaint();
    }
}
