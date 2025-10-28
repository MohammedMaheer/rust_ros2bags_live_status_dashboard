use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Real-time metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MetricsSnapshot {
    pub timestamp: u128,
    pub cpu_percent: f32,
    pub memory_mb: f32,
    pub disk_free_gb: f32,
    pub message_rate_hz: f32,
    pub storage_used_mb: f32,
    pub active_topics: usize,
    pub network_latency_ms: f32,
    pub upload_bandwidth_mbps: f32,
}

/// Circular history buffer for metrics
#[allow(dead_code)]
pub struct MetricsCollector {
    history: Arc<Mutex<VecDeque<MetricsSnapshot>>>,
    max_history: usize,
}

impl MetricsCollector {
    #[allow(dead_code)]
    pub fn new(max_history: usize) -> Self {
        MetricsCollector {
            history: Arc::new(Mutex::new(VecDeque::with_capacity(max_history))),
            max_history,
        }
    }

    #[allow(dead_code)]
    pub async fn record_snapshot(&self, snapshot: MetricsSnapshot) {
        let mut history = self.history.lock().await;
        history.push_back(snapshot);
        if history.len() > self.max_history {
            history.pop_front();
        }
    }

    #[allow(dead_code)]
    pub async fn get_history(&self) -> Vec<MetricsSnapshot> {
        self.history.lock().await.iter().cloned().collect()
    }

    #[allow(dead_code)]
    pub async fn get_latest(&self) -> Option<MetricsSnapshot> {
        self.history.lock().await.back().cloned()
    }

    #[allow(dead_code)]
    pub async fn get_average(&self) -> Option<MetricsSnapshot> {
        let history = self.history.lock().await;
        if history.is_empty() {
            return None;
        }

        let count = history.len() as f32;
        let avg = MetricsSnapshot {
            timestamp: history.back().map(|s| s.timestamp).unwrap_or(0),
            cpu_percent: history.iter().map(|s| s.cpu_percent).sum::<f32>() / count,
            memory_mb: history.iter().map(|s| s.memory_mb).sum::<f32>() / count,
            disk_free_gb: history.back().map(|s| s.disk_free_gb).unwrap_or(0.0),
            message_rate_hz: history.iter().map(|s| s.message_rate_hz).sum::<f32>() / count,
            storage_used_mb: history.back().map(|s| s.storage_used_mb).unwrap_or(0.0),
            active_topics: history.back().map(|s| s.active_topics).unwrap_or(0),
            network_latency_ms: history.iter().map(|s| s.network_latency_ms).sum::<f32>() / count,
            upload_bandwidth_mbps: history.iter().map(|s| s.upload_bandwidth_mbps).sum::<f32>() / count,
        };

        Some(avg)
    }
}

#[allow(dead_code)]
pub async fn start_metrics_server(_bind: &str) -> Result<()> {
    // Placeholder for Prometheus metrics endpoint
    tracing::info!("metrics server would start at {}", _bind);
    Ok(())
}

pub fn detect_ros2_available() -> bool {
    // Try to detect ROS2 environment
    let ros_distro_ok = std::env::var("ROS_DISTRO").is_ok();
    let ros_domain_ok = std::env::var("ROS_DOMAIN_ID").is_ok();
    let available = ros_distro_ok || ros_domain_ok;
    tracing::info!("detect_ros2_available: ROS_DISTRO={}, ROS_DOMAIN_ID={}, result={}", ros_distro_ok, ros_domain_ok, available);
    available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_history() {
        let collector = MetricsCollector::new(10);

        for i in 0..5 {
            let snap = MetricsSnapshot {
                timestamp: i as u128,
                cpu_percent: i as f32 * 10.0,
                memory_mb: 512.0 + i as f32,
                disk_free_gb: 500.0,
                message_rate_hz: 50.0 + i as f32,
                storage_used_mb: 100.0 + i as f32 * 10.0,
                active_topics: 20 + i,
                network_latency_ms: 10.0 + i as f32,
                upload_bandwidth_mbps: 5.0 + i as f32,
            };
            collector.record_snapshot(snap).await;
        }

        let history = collector.get_history().await;
        assert_eq!(history.len(), 5);
        assert_eq!(history[0].cpu_percent, 0.0);
        assert_eq!(history[4].cpu_percent, 40.0);
    }

    #[tokio::test]
    async fn test_metrics_average() {
        let collector = MetricsCollector::new(10);

        for i in 0..10 {
            let snap = MetricsSnapshot {
                timestamp: i as u128,
                cpu_percent: 50.0,
                memory_mb: 500.0,
                disk_free_gb: 400.0,
                message_rate_hz: 100.0,
                storage_used_mb: 200.0,
                active_topics: 20,
                network_latency_ms: 15.0,
                upload_bandwidth_mbps: 10.0,
            };
            collector.record_snapshot(snap).await;
        }

        let avg = collector.get_average().await.unwrap();
        assert_eq!(avg.cpu_percent, 50.0);
        assert_eq!(avg.message_rate_hz, 100.0);
    }
}
