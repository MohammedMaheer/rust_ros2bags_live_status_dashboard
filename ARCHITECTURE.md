# Architecture & Design

## System Overview

```
┌─────────────────────────────────────────────────────────────┐
│           ROS2 Multi-Robot Recorder Dashboard              │
└─────────────────────────────────────────────────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
   ┌─────────┐        ┌─────────┐      ┌──────────┐
   │Recorder │        │ Storage │      │Dashboard │
   │(ROS2)   │        │ (WAL)   │      │ (egui)   │
   └────┬────┘        └────┬────┘      └──────────┘
        │                  │
        └──────────┬───────┘
                   ▼
           ┌──────────────┐
           │ Sync Daemon  │
           │(background)  │
           └──────────────┘
                   │
                   ▼
            ┌────────────┐
            │ S3 Cloud   │
            │  Bucket    │
            └────────────┘
```

## Core Modules

### 1. Recorder (`recorder.rs`)

**Purpose**: Subscribe to ROS2 topics and collect messages

**Design**:
- Conditional compilation: `#[cfg(feature = "ros2")]` for real ROS2 integration
- Falls back to mock recorder when ROS2 unavailable
- Maintains `RecorderState` with atomic message counter
- Async task spawned on startup

**Key Functions**:
```rust
pub fn start_recorder(storage: Storage, cfg: AppConfig) -> JoinHandle<()>
```

**ROS2 Integration** (when feature enabled):
- Creates r2r context for DDS access
- Discovers topics via graph API: `graph.get_topic_names_and_types()`
- Subscribes to each topic dynamically
- Spins node event loop with 100ms timeout
- Calls `storage.append_record()` for each message

**Mock Mode**:
- Simulates 4 topics: `/sensor/lidar`, `/tf`, `/odometry`, `/diagnostics`
- Records 2 robots: `robot1`, `robot2`
- 50 Hz simulation rate

### 2. Storage (`storage.rs`)

**Purpose**: Crash-safe recording with write-ahead logging (WAL)

**Design Principles**:
- **Append-only**: All writes are sequential appends to segment files
- **Atomic commits**: Each record has framing, metadata, CRC32, and fsync
- **Crash recovery**: Checkpoint manifests enable resume from last good state
- **Segment rotation**: Fixed-size segments (16 MiB default) for manageability

**Record Format**:
```
┌──────────┬───────────┬──────────┬─────────────────┬────────────┐
│  Magic   │ Meta Len  │ Metadata │ Payload Length  │  Payload   │
│(0xDEAD   │  (u32 LE) │ (JSON)   │ & CRC32 & ts    │ (bytes)    │
│  BEEF)   │           │          │                 │            │
└──────────┴───────────┴──────────┴─────────────────┴────────────┘
  4 bytes     4 bytes   variable    JSON metadata     variable
```

**Metadata JSON**:
```json
{
  "magic": 3735928559,
  "timestamp": 1729069234567,
  "topic": "/sensor/lidar",
  "namespace": "robot1",
  "payload_len": 2048,
  "payload_crc32": "0xabcd1234"
}
```

**Recovery Algorithm**:
1. Load `.checkpoint` file (JSON with `current_segment` number)
2. Open that segment file
3. Read records sequentially
4. If magic != 0xDEADBEEF or CRC mismatch, stop (corrupted tail)
5. Replay all valid records to in-memory buffer
6. Resume from next segment

**Methods**:
- `new(cfg)` - Initialize, recover from checkpoint
- `append_record(topic, ns, data, ts)` - Write with fsync
- `rotate_segment()` - Save checkpoint, move to next segment
- `list_segments()` - Get all pending segments
- `segment_checksum(path)` - SHA256 of segment file
- `replay_segment(path)` - Read all records from segment

**Thread Safety**:
- Uses `tokio::sync::Mutex` for inner state
- Segment rotation is atomic (checkpoint write before increment)

### 3. Sync Daemon (`sync.rs`)

**Purpose**: Background upload to cloud with resumable state

**Design**:
- Independent background task
- Maintains upload queue (Vec<UploadState>)
- Exponential backoff on failures
- Persisted resume state for each segment

**Upload Flow**:
```
Segment File (16 MB)
       │
       ▼
    Chunker (chunks = 16 segments @ 1 MB each)
       │
       ├─► Chunk 0 (1 MB) ──SHA256──► sha256_0
       ├─► Chunk 1 (1 MB) ──SHA256──► sha256_1
       └─► Chunk N (...)  ──SHA256──► sha256_N
       │
       ▼
   Upload Queue (persisted to disk)
       │
       ├─► { segment, sha256, [uploaded_chunks], timestamp }
       └─► ...
       │
       ▼
   S3 Multipart Upload (mock impl in v0.1.0)
```

**Resume State** (JSON):
```json
{
  "segment_path": "./data/segment-0.log",
  "segment_sha256": "abc123...",
  "chunks_uploaded": [
    { "chunk_index": 0, "sha256": "def456...", "upload_id": "..." },
    { "chunk_index": 1, "sha256": "ghi789...", "upload_id": "..." }
  ],
  "timestamp": 1729069234567
}
```

**Backoff Strategy**:
- Retry 1: 2 seconds
- Retry 2: 4 seconds
- Retry 3: 8 seconds
- ...
- Retry 7: 128 seconds (capped at 120)

**Methods**:
- `new(storage, config)` - Initialize daemon
- `get_status()` - Return current SyncStatus
- `queue_segment(path)` - Add to upload queue
- `sync_loop(max_retries)` - Main background loop
- `process_next_upload()` - Handle one segment

**Status Tracking**:
```rust
pub struct SyncStatus {
    pub is_syncing: bool,
    pub last_sync_time: Option<u128>,
    pub upload_errors: usize,
    pub total_segments_synced: usize,
}
```

### 4. Dashboard (`dashboard.rs`)

**Purpose**: Live UI for monitoring and control

**UI Layout** (egui):
```
┌──────────────────────────────────────────────────────┐
│ ROS2 Multi-Robot Recorder & Dashboard                │
├──────────────────────────────────────────────────────┤
│ Recording: ● ACTIVE  │  Rate: 150.5 Hz  │  Storage: 234.2 MB  │
├──────────────────────────────────────────────────────┤
│ Select Robot: [robot1 ▼]                             │
├──────────────────────────────────────────────────────┤
│ Recording Controls                                    │
│ [▶ Start] [⏸ Pause] [⏹ Stop] | [↑ Sync] [📊 Export] │
├──────────────────────────────────────────────────────┤
│ Sync Status                                           │
│ Status: Syncing...      Last Sync: 2 min ago         │
│ Upload Progress: [=====────────────] 65%              │
├──────────────────────────────────────────────────────┤
│ Topics (20 active)                                    │
│ /sensor/lidar ◆ 50 Hz                                │
│ /tf           ◆ 100 Hz                               │
│ /odometry     ◆ 25 Hz                                │
├──────────────────────────────────────────────────────┤
│ System Diagnostics                                    │
│ CPU: 45%  │  Memory: 1.2 GB  │  Disk: 234/1000 GB    │
│ Network: ● Online  Latency: 12 ms                    │
└──────────────────────────────────────────────────────┘
```

**Features**:
- Real-time metrics from mock data
- Responsive controls (30 FPS egui loop)
- Multi-robot selector
- Status polling with `ctx.request_repaint()`
- Future: Live graph integration with plotters crate

**Conditional Compilation**:
- `#[cfg(feature = "ui")]` for egui (macOS/Windows/Linux)
- Falls back to headless mode when UI feature disabled

### 5. Exporter (`exporter.rs`)

**Purpose**: ML-ready data export

**Supported Formats**:
- **Parquet**: Arrow2-based columnar format (future)
- **CSV**: Comma-separated values (future)
- **TFRecord**: TensorFlow record format (stub)
- **Numpy**: .npy binary format (stub)

**Manifest Generation**:
```json
{
  "export_id": "session_123-parquet",
  "format": "Parquet",
  "timestamp_utc": 1729069234567,
  "num_records": 50000,
  "topics": [
    {
      "topic": "/sensor/lidar",
      "message_type": "sensor_msgs/PointCloud2",
      "sample_count": 10000,
      "sample_rate_hz": 50.0
    }
  ]
}
```

**Methods**:
- `export_session(session_id, output_dir, format)` - Main export
- `export_to_parquet()` - Columnar format
- `export_to_csv()` - Row format
- `export_to_tfrecord()` - TensorFlow format
- `export_to_numpy()` - Numpy array format

### 6. Configuration (`config.rs`)

**Storage Config**:
- `path`: Local data directory
- `wal_segment_size`: Segment rotation threshold
- `compress`: Enable compression (future)
- `encryption`: Encryption mode (future)

**Sync Config**:
- `endpoint`: S3-compatible endpoint URL
- `bucket`: Cloud bucket name
- `chunk_size`: Upload chunk size (16 MiB default)
- `max_retries`: Exponential backoff retries

## Concurrency Model

```
┌─────────────────────────────────────────┐
│         main() tokio runtime            │
├─────────────────────────────────────────┤
│                                         │
│  Recorder Task ◄──┐                    │
│  (async)          │  Arc<Storage>      │
│   - ROS2 loop     │   (thread-safe)    │
│   - append_record ─────┼────┐          │
│                  │          │          │
│  Sync Daemon Task         │  Disk      │
│  (background)             │  I/O       │
│   - queue check ──────────┘  (async)   │
│   - upload                            │
│                                         │
│  Dashboard Task                        │
│  (blocking UI)                         │
│   - egui event loop                    │
│   - (blocks on UI exit)               │
│                                         │
└─────────────────────────────────────────┘
```

**Synchronization Primitives**:
- `Arc<Mutex<T>>` for shared state (Storage inner, SyncDaemon)
- `Arc<AtomicU64>` for message counters (non-blocking)
- `tokio::sync::Mutex` for async locks

## Error Handling

**Strategy**: All operations return `anyhow::Result<T>` with context

**Common Errors**:
- IO errors (disk full, permission denied)
- Serialization errors (JSON, frame parsing)
- Checksum mismatches (CRC32, SHA256)
- Network errors (timeout, unreachable)

**Propagation**:
```rust
match storage.append_record(...).await {
    Ok(_) => tracing::debug!("recorded"),
    Err(e) => {
        tracing::error!("record failed: {:#?}", e);
        // Continue or fail depending on policy
    }
}
```

## Performance Characteristics

| Metric | Target | Achieved |
|--------|--------|----------|
| Recording throughput | >100 MB/s | N/A (untested at scale) |
| WAL commit latency | <1 ms | fsync-limited (~1-2 ms on SSD) |
| Message overhead | <50 bytes | JSON metadata + framing |
| UI frame rate | 30 FPS | 30 FPS (egui) |
| Sync daemon CPU | <5% | N/A (mock impl) |
| Memory baseline | <100 MB | ~50 MB measured |

## Future Enhancements

### Short Term
- [ ] Real S3 multipart upload integration
- [ ] Parquet/CSV export with arrow2
- [ ] Prometheus metrics exporter
- [ ] AES-GCM encryption

### Medium Term
- [ ] Distributed recording (multiple recorders syncing to central)
- [ ] Time-series database (e.g., InfluxDB) integration
- [ ] Web dashboard (replace egui with web UI)
- [ ] Kubernetes deployment

### Long Term
- [ ] Machine learning inference pipeline
- [ ] Real-time anomaly detection
- [ ] Multi-site replication
- [ ] Commercial offering

---

**Last Updated**: 2025-10-27
**Version**: 0.1.0
