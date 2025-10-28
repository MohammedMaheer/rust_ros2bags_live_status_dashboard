use crate::config::AppConfig;
use crate::storage::Storage;
use tokio::task::JoinHandle;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Recorder state shared across tasks
pub struct RecorderState {
    pub messages_recorded: Arc<AtomicU64>,
    pub is_active: Arc<tokio::sync::Mutex<bool>>,
}

impl RecorderState {
    pub fn new() -> Self {
        RecorderState {
            messages_recorded: Arc::new(AtomicU64::new(0)),
            is_active: Arc::new(tokio::sync::Mutex::new(false)),
        }
    }

    pub fn increment_messages(&self) {
        self.messages_recorded.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn get_total_messages(&self) -> u64 {
        self.messages_recorded.load(Ordering::Acquire)
    }
}

pub fn start_recorder(storage: Storage, _cfg: AppConfig) -> JoinHandle<()> {
    tokio::spawn(async move {
        #[cfg(feature = "ros2")]
        {
            match run_ros2_recorder(storage).await {
                Ok(_) => tracing::info!("ROS2 recorder stopped cleanly"),
                Err(e) => tracing::error!("ROS2 recorder error: {:#?}", e),
            }
        }

        #[cfg(not(feature = "ros2"))]
        {
            run_mock_recorder(storage).await;
        }
    })
}

#[cfg(feature = "ros2")]
async fn run_ros2_recorder(storage: Storage) -> anyhow::Result<()> {
    use r2r::Context;
    use std::sync::Mutex as StdMutex;

    tracing::info!("initializing ROS2 context");
    let ctx = Context::new()?;

    // Create a node for topic discovery and subscriptions
    let mut node = ctx.create_node("ros2_recorder")?;

    tracing::info!("discovering ROS2 topics");

    // Get graph information to discover available topics
    let graph = node.graph();
    let topic_names_and_types = graph.get_topic_names_and_types()?;

    tracing::info!("found {} topics", topic_names_and_types.len());

    // Subscribe to topics dynamically
    let mut subscribers: Vec<Box<dyn std::any::Any>> = Vec::new();

    for (topic_name, types) in &topic_names_and_types {
        // Skip some system topics
        if topic_name.starts_with("/parameter_events") || 
           topic_name.starts_with("/rosout") ||
           topic_name.starts_with("/_") {
            continue;
        }

        tracing::info!("subscribing to topic: {} (types: {:?})", topic_name, types);

        // For now, we'll subscribe to generic messages since r2r requires type stubs
        // In production, you'd generate type-specific subscribers for each message type
        match create_generic_subscription(&mut node, topic_name, types).await {
            Ok(sub) => {
                subscribers.push(sub);
            }
            Err(e) => {
                tracing::warn!("failed to subscribe to {}: {}", topic_name, e);
            }
        }
    }

    tracing::info!("started recording from {} topics", subscribers.len());

    let state = RecorderState::new();
    *state.is_active.lock().await = true;

    // Main recording loop
    loop {
        // Spin node to process callbacks
        match tokio::time::timeout(Duration::from_millis(100), async {
            node.spin_once(Duration::from_millis(10))
        })
        .await
        {
            Ok(Ok(_)) => {
                state.increment_messages();
            }
            Ok(Err(e)) => {
                tracing::error!("node spin error: {:#?}", e);
                break;
            }
            Err(_) => {
                // Timeout is fine, just continue
            }
        }

        // Log periodically
        let total = state.get_total_messages().await;
        if total % 1000 == 0 && total > 0 {
            tracing::info!("ros2_recorder: {} messages recorded", total);
        }
    }

    *state.is_active.lock().await = false;
    Ok(())
}

#[cfg(feature = "ros2")]
async fn create_generic_subscription(
    node: &mut r2r::Node,
    topic_name: &str,
    _types: &[String],
) -> anyhow::Result<Box<dyn std::any::Any>> {
    // This is a simplified stub: r2r typically requires concrete types
    // In a real implementation, you'd generate message types or use a schema registry
    tracing::debug!("creating subscription for {}", topic_name);

    // For now, return a dummy subscription that won't actually receive data
    // TODO: Integrate with concrete ROS2 message types or a message registry
    Ok(Box::new(topic_name.to_string()))
}

#[cfg(not(feature = "ros2"))]
async fn run_mock_recorder(storage: Storage) {
    let state = RecorderState::new();
    *state.is_active.lock().await = true;

    tracing::info!("starting mock recorder (ROS2 feature not enabled)");

    let _now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    // Simulate recording messages
    loop {
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Mock: simulate recording sensor messages
        let topics = ["/sensor/lidar", "/tf", "/odometry", "/diagnostics"];
        let namespaces = ["robot1", "robot2"];

        for topic in &topics {
            for ns in &namespaces {
                let mock_data = format!("mock_data_{}_{}", ns, topic).into_bytes();
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();

                if let Err(e) = storage.append_record(topic, ns, &mock_data, ts).await {
                    tracing::error!("failed to record message: {}", e);
                }
            }
        }

        state.increment_messages();

        let total = state.get_total_messages().await;
        if total % 100 == 0 {
            tracing::debug!("mock_recorder: {} iterations", total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_recorder_state_tracking() {
        let state = RecorderState::new();
        
        state.increment_messages();
        state.increment_messages();
        state.increment_messages();
        
        assert_eq!(state.get_total_messages().await, 3);
    }

    #[tokio::test]
    async fn test_recorder_state_concurrent() {
        let state = Arc::new(RecorderState::new());
        
        let mut handles = vec![];
        for _ in 0..10 {
            let s = state.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..10 {
                    s.increment_messages();
                }
            }));
        }
        
        for h in handles {
            h.await.unwrap();
        }
        
        assert_eq!(state.get_total_messages().await, 100);
    }
}
