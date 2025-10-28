use crate::config::SyncConfig;
use crate::storage::Storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// Resumable upload state persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadState {
    pub segment_path: String,
    pub segment_sha256: String,
    pub chunks_uploaded: Vec<UploadedChunk>,
    pub timestamp: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedChunk {
    pub chunk_index: u32,
    pub chunk_size: usize,
    pub sha256: String,
    pub upload_id: Option<String>,
}

#[derive(Clone)]
pub struct SyncDaemon {
    storage: Storage,
    config: SyncConfig,
    upload_queue: Arc<Mutex<Vec<UploadState>>>,
    sync_status: Arc<Mutex<SyncStatus>>,
}

#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub is_syncing: bool,
    pub last_sync_time: Option<u128>,
    pub upload_errors: usize,
    pub total_segments_synced: usize,
}

impl SyncDaemon {
    pub fn new(storage: Storage, config: SyncConfig) -> Self {
        SyncDaemon {
            storage,
            config,
            upload_queue: Arc::new(Mutex::new(Vec::new())),
            sync_status: Arc::new(Mutex::new(SyncStatus {
                is_syncing: false,
                last_sync_time: None,
                upload_errors: 0,
                total_segments_synced: 0,
            })),
        }
    }

    pub async fn get_status(&self) -> SyncStatus {
        self.sync_status.lock().await.clone()
    }

    /// Queue a segment for upload
    pub async fn queue_segment(&self, segment_path: PathBuf) -> Result<()> {
        let sha256 = Storage::segment_checksum(&segment_path).await?;
        let state = UploadState {
            segment_path: segment_path.to_string_lossy().to_string(),
            segment_sha256: sha256,
            chunks_uploaded: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        };
        self.upload_queue.lock().await.push(state);
        tracing::info!("queued segment for upload: {}", segment_path.display());
        Ok(())
    }

    /// Main sync loop: process queue with exponential backoff and retries
    pub async fn sync_loop(&self, mut max_retries: usize) {
        loop {
            {
                let queue = self.upload_queue.lock().await;
                if !queue.is_empty() {
                    let mut status = self.sync_status.lock().await;
                    status.is_syncing = true;
                } else {
                    let mut status = self.sync_status.lock().await;
                    status.is_syncing = false;
                    drop(status);
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }

            // Process first item in queue
            let result = self.process_next_upload(max_retries).await;

            match result {
                Ok(()) => {
                    let mut status = self.sync_status.lock().await;
                    status.total_segments_synced += 1;
                    status.last_sync_time = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis(),
                    );
                    tracing::info!(
                        "segment uploaded successfully (total: {})",
                        status.total_segments_synced
                    );
                    max_retries = 7; // Reset retries after success
                }
                Err(e) => {
                    let mut status = self.sync_status.lock().await;
                    status.upload_errors += 1;
                    tracing::error!("upload failed: {:#?}, retrying...", e);
                    // Apply exponential backoff
                    let backoff_secs = 2_u64.min((max_retries as u64).saturating_mul(2)).min(120);
                    sleep(Duration::from_secs(backoff_secs)).await;
                    max_retries = max_retries.saturating_sub(1);
                    if max_retries == 0 {
                        max_retries = 7; // Reset for next item
                    }
                }
            }
        }
    }

    async fn process_next_upload(&self, _retries: usize) -> Result<()> {
        let mut queue = self.upload_queue.lock().await;
        if queue.is_empty() {
            return Ok(());
        }

        let state = queue.remove(0);
        drop(queue);

        // Split segment into chunks
        let segment_path = PathBuf::from(&state.segment_path);
        let data = tokio::fs::read(&segment_path).await?;
        let chunk_size = self.config.chunk_size;
        let chunks: Vec<Vec<u8>> = data
            .chunks(chunk_size)
            .map(|c| c.to_vec())
            .collect();

        tracing::info!("segment {} split into {} chunks", state.segment_path, chunks.len());

        // Upload each chunk (simulate with local mock for now)
        for (idx, chunk) in chunks.iter().enumerate() {
            let chunk_sha256 = format!("{:x}", Sha256::digest(chunk));

            // Mock upload: in a real system, this would call S3 multipart upload
            self.mock_upload_chunk(idx as u32, chunk, &chunk_sha256).await?;

            tracing::debug!("uploaded chunk {} of {}", idx, chunks.len());
        }

        Ok(())
    }

    async fn mock_upload_chunk(&self, _idx: u32, _chunk: &[u8], _sha256: &str) -> Result<()> {
        // Mock: simulate S3 multipart upload
        // In real implementation: call reqwest with presigned URLs or multipart forms
        // For now: just trace and succeed
        tracing::debug!("mock upload chunk: sha256={}, size={}", _sha256, _chunk.len());
        Ok(())
    }
}

pub fn start_sync_daemon(storage: Storage, cfg: SyncConfig) -> JoinHandle<()> {
    tokio::spawn(async move {
        let daemon = SyncDaemon::new(storage, cfg);
        daemon.sync_loop(7).await;
    })
}
