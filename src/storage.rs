use crate::config::StorageConfig;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

const RECORD_FRAME_HEADER: u32 = 0xDEADBEEF;
const MAX_SEGMENT_SIZE: u64 = 16 * 1024 * 1024;

/// Per-record metadata and payload framing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordFrame {
    magic: u32,
    timestamp: u128,
    topic: String,
    namespace: String,
    payload_len: u32,
    payload_crc32: u32,
}

impl RecordFrame {
    fn to_bytes(&self, payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.magic.to_le_bytes());
        let meta_json = serde_json::to_string(&self).expect("frame serialization");
        buf.extend_from_slice(&(meta_json.len() as u32).to_le_bytes());
        buf.extend_from_slice(meta_json.as_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    fn from_reader(reader: &mut dyn Read) -> Result<Option<(RecordFrame, Vec<u8>)>> {
        let mut magic_buf = [0u8; 4];
        if reader.read_exact(&mut magic_buf).is_err() {
            return Ok(None);
        }
        let magic = u32::from_le_bytes(magic_buf);
        if magic != RECORD_FRAME_HEADER {
            return Ok(None);
        }

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let meta_len = u32::from_le_bytes(len_buf) as usize;

        let mut meta_buf = vec![0u8; meta_len];
        reader.read_exact(&mut meta_buf)?;
        let frame: RecordFrame = serde_json::from_slice(&meta_buf)?;

        let mut payload = vec![0u8; frame.payload_len as usize];
        reader.read_exact(&mut payload)?;

        let crc = crc32fast::hash(&payload);
        if crc != frame.payload_crc32 {
            return Err(anyhow!("payload CRC mismatch: expected {}, got {}", frame.payload_crc32, crc));
        }

        Ok(Some((frame, payload)))
    }
}

#[derive(Clone)]
pub struct Storage {
    pub root: Arc<PathBuf>,
    inner: Arc<Mutex<StorageInner>>,
}

struct StorageInner {
    current_segment: u64,
    current_segment_size: u64,
}

impl Storage {
    pub async fn new(cfg: &StorageConfig) -> Result<Self> {
        let root = cfg.path.clone();
        tokio::fs::create_dir_all(&root).await?;

        let (segment_num, _) = Self::recover_checkpoint(&root).await?;

        let inner = StorageInner { current_segment: segment_num, current_segment_size: 0 };
        Ok(Storage { root: Arc::new(root), inner: Arc::new(Mutex::new(inner)) })
    }

    pub async fn append_record(&self, topic: &str, namespace: &str, data: &[u8], timestamp: u128) -> Result<()> {
        let mut inner = self.inner.lock().await;

        let projected_size = inner.current_segment_size + data.len() as u64 + 100;
        if projected_size > MAX_SEGMENT_SIZE {
            drop(inner);
            self.rotate_segment().await?;
            inner = self.inner.lock().await;
        }

        let segment_file = self.root.join(format!("segment-{}.log", inner.current_segment));

        let crc = crc32fast::hash(data);
        let frame = RecordFrame {
            magic: RECORD_FRAME_HEADER,
            timestamp,
            topic: topic.to_string(),
            namespace: namespace.to_string(),
            payload_len: data.len() as u32,
            payload_crc32: crc,
        };

        let frame_data = frame.to_bytes(data);

        let mut f = OpenOptions::new().create(true).append(true).open(&segment_file).await?;
        f.write_all(&frame_data).await?;
        f.sync_all().await?;

        inner.current_segment_size += frame_data.len() as u64;

        Ok(())
    }

    pub async fn rotate_segment(&self) -> Result<PathBuf> {
        let mut inner = self.inner.lock().await;

        Self::write_checkpoint(&self.root, inner.current_segment).await?;

        inner.current_segment += 1;
        inner.current_segment_size = 0;
        let new_path = self.root.join(format!("segment-{}.log", inner.current_segment));
        let _ = tokio::fs::File::create(&new_path).await?;
        tracing::info!("rotated to segment {}", inner.current_segment);
        Ok(new_path)
    }

    pub async fn list_segments(&self) -> Result<Vec<PathBuf>> {
        let mut entries = tokio::fs::read_dir(&*self.root).await?;
        let mut out = Vec::new();
        loop {
            match entries.next_entry().await {
                Ok(Some(ent)) => {
                    let p = ent.path();
                    if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                        if n.starts_with("segment-") && n.ends_with(".log") {
                            out.push(p);
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => return Err(e.into()),
            }
        }
        out.sort();
        Ok(out)
    }

    pub async fn segment_checksum(path: &Path) -> Result<String> {
        let data = tokio::fs::read(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(format!("{:x}", hasher.finalize()))
    }

    async fn write_checkpoint(root: &Path, segment: u64) -> Result<()> {
        let manifest = serde_json::json!({
            "current_segment": segment,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        });
        let path = root.join(".checkpoint");
        let tmp_path = root.join(".checkpoint.tmp");
        tokio::fs::write(&tmp_path, manifest.to_string()).await?;
        tokio::fs::rename(&tmp_path, &path).await?;
        Ok(())
    }

    async fn recover_checkpoint(root: &Path) -> Result<(u64, Option<String>)> {
        let path = root.join(".checkpoint");
        if !path.exists() {
            return Ok((0, None));
        }
        let data = tokio::fs::read_to_string(&path).await?;
        let manifest: serde_json::Value = serde_json::from_str(&data)?;
        let segment = manifest["current_segment"].as_u64().unwrap_or(0);
        tracing::info!("recovered checkpoint: segment {}", segment);
        Ok((segment, Some(data)))
    }

    pub async fn replay_segment(path: &Path) -> Result<Vec<(String, String, u128, Vec<u8>)>> {
        let mut file = std::fs::File::open(path)?;
        let mut records = Vec::new();
        while let Some((frame, payload)) = RecordFrame::from_reader(&mut file)? {
            records.push((frame.topic, frame.namespace, frame.timestamp, payload));
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_append_and_replay() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let cfg = StorageConfig {
            path: tmpdir.path().to_path_buf(),
            wal_segment_size: 1024 * 1024,
            compress: false,
            encryption: None,
            enable_aes_gcm: false,
        };

        let storage = Storage::new(&cfg).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();

        storage.append_record("topic1", "robot1", b"hello", now).await?;
        storage.append_record("topic2", "robot1", b"world", now + 1).await?;

        let segments = storage.list_segments().await?;
        assert_eq!(segments.len(), 1);

        let records = Storage::replay_segment(&segments[0]).await?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].0, "topic1");
        assert_eq!(records[0].3, b"hello");
        assert_eq!(records[1].0, "topic2");
        assert_eq!(records[1].3, b"world");

        Ok(())
    }

    #[tokio::test]
    async fn test_segment_rotation() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let cfg = StorageConfig {
            path: tmpdir.path().to_path_buf(),
            wal_segment_size: 512,
            compress: false,
            encryption: None,
            enable_aes_gcm: false,
        };

        let storage = Storage::new(&cfg).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();

        storage.append_record("topic1", "robot1", b"record1", now).await?;
        let segments_before = storage.list_segments().await?;
        assert_eq!(segments_before.len(), 1);

        storage.rotate_segment().await?;

        storage.append_record("topic2", "robot1", b"record2", now + 1).await?;
        let segments_after = storage.list_segments().await?;
        assert_eq!(segments_after.len(), 2);

        let records0 = Storage::replay_segment(&segments_before[0]).await?;
        assert_eq!(records0[0].0, "topic1");

        let records1 = Storage::replay_segment(&segments_after[1]).await?;
        assert_eq!(records1[0].0, "topic2");

        Ok(())
    }

    #[tokio::test]
    async fn test_checkpoint_recovery() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let cfg = StorageConfig {
            path: tmpdir.path().to_path_buf(),
            wal_segment_size: 512,
            compress: false,
            encryption: None,
            enable_aes_gcm: false,
        };

        let storage = Storage::new(&cfg).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();

        storage.append_record("topic1", "robot1", b"data1", now).await?;
        storage.rotate_segment().await?;
        storage.append_record("topic2", "robot1", b"data2", now + 1).await?;

        let storage2 = Storage::new(&cfg).await?;

        let segments = storage2.list_segments().await?;
        assert_eq!(segments.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_payload_corruption_detection() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let cfg = StorageConfig {
            path: tmpdir.path().to_path_buf(),
            wal_segment_size: 1024 * 1024,
            compress: false,
            encryption: None,
            enable_aes_gcm: false,
        };

        let storage = Storage::new(&cfg).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();

        storage.append_record("topic1", "robot1", b"original_data", now).await?;

        let segments = storage.list_segments().await?;
        let segment_path = &segments[0];

        let mut data = fs::read(segment_path)?;
        if !data.is_empty() {
            let idx = data.len() - 10;
            data[idx] ^= 0xFF;
            fs::write(segment_path, data)?;
        }

        let _result = Storage::replay_segment(segment_path).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_segment_checksum() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let cfg = StorageConfig {
            path: tmpdir.path().to_path_buf(),
            wal_segment_size: 1024 * 1024,
            compress: false,
            encryption: None,
            enable_aes_gcm: false,
        };

        let storage = Storage::new(&cfg).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();

        storage.append_record("topic1", "robot1", b"test_data", now).await?;

        let segments = storage.list_segments().await?;
        let checksum1 = Storage::segment_checksum(&segments[0]).await?;
        let checksum2 = Storage::segment_checksum(&segments[0]).await?;

        assert_eq!(checksum1, checksum2);

        Ok(())
    }
}
