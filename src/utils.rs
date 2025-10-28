use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicManifestEntry {
    pub topic: String,
    pub msg_type: String,
    pub sample_rate_hz: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub recording_id: String,
    pub start_time_unix_ms: u128,
    pub end_time_unix_ms: Option<u128>,
    pub topics: Vec<TopicManifestEntry>,
}
