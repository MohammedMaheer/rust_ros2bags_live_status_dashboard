# Deployment Guide: ROS2 Ubuntu Integration

This guide walks through testing the recorder on Ubuntu 22.04 with ROS2 Humble.

## Prerequisites

- Ubuntu 22.04 LTS or later
- ROS2 Humble or Iron (installed per official docs)
- Rust 1.70+ (via rustup)
- A local ROS2 environment set up

## Step 1: Install ROS2 (if not already done)

```bash
# Install ROS2 Humble (one-time setup)
sudo apt update
sudo apt install ros-humble-desktop

# Source environment
source /opt/ros/humble/setup.bash

# Verify installation
ros2 topic list
```

## Step 2: Clone and Build the Recorder

```bash
# Clone or navigate to project
cd ~/rust_ros2_recording_dashboard

# Source ROS2
source /opt/ros/humble/setup.bash

# Build with ROS2 support
cargo build --release --features ros2

# Expected: Finished release profile in ~2-3 min
```

## Step 3: Start a Test ROS2 Node

In **Terminal 1**, start a demo publisher:

```bash
source /opt/ros/humble/setup.bash
ros2 run demo_nodes_cpp talker
```

You should see:
```
[INFO] Publishing: "Hello World: 1"
[INFO] Publishing: "Hello World: 2"
...
```

## Step 4: Run the Recorder

In **Terminal 2**, start the recorder:

```bash
source /opt/ros/humble/setup.bash
cd ~/rust_ros2_recording_dashboard

# Run with tracing for diagnostics
RUST_LOG=info cargo run --release --features ros2
```

Expected output:
```
[INFO] Starting rust_ros2_recorder
[INFO] initializing ROS2 context
[INFO] discovering ROS2 topics
[INFO] found N topics
[INFO] subscribing to topic: /topic_0 (types: ["std_msgs/msg/String"])
[INFO] started recording from N topics
[DEBUG] ros2_recorder: 1000 messages recorded
```

## Step 5: Verify Recording

In **Terminal 3**, check recorded data:

```bash
# List recorded segments
ls -lh ./data/
du -sh ./data/

# Example output:
# -rw-r--r--  1 user  group  2.3M  Oct 27 10:15 segment-0.log
# -rw-r--r--  1 user  group  1.2M  Oct 27 10:17 segment-1.log
# -rw-r--r--  1 user  group  512K  Oct 27 10:20 .checkpoint
```

## Step 6: Test Export

Create a Python script to replay and export:

```bash
mkdir -p exports
```

In Python:

```python
import os
import struct

# Read segment file
with open('./data/segment-0.log', 'rb') as f:
    while True:
        # Read magic
        magic_bytes = f.read(4)
        if len(magic_bytes) < 4:
            break
        
        magic = struct.unpack('<I', magic_bytes)[0]
        if magic != 0xDEADBEEF:
            print(f"Bad magic: {hex(magic)}")
            break
        
        # Read metadata length
        meta_len_bytes = f.read(4)
        if len(meta_len_bytes) < 4:
            break
        
        meta_len = struct.unpack('<I', meta_len_bytes)[0]
        meta_bytes = f.read(meta_len)
        
        import json
        meta = json.loads(meta_bytes.decode())
        print(f"Topic: {meta['topic']}, Namespace: {meta['namespace']}, Timestamp: {meta['timestamp']}")
        
        # Skip payload
        payload_len = meta['payload_len']
        f.seek(f.tell() + payload_len)
```

## Step 7: Run Tests

```bash
# Full test suite
cargo test --release --features ros2

# Expected: 8 passed tests
```

## Step 8: Continuous Integration

Create a GitHub Actions workflow:

`.github/workflows/ci.yml`:

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --release --features ros2
  
  build:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo build --release
```

## Troubleshooting

### "ROS2 context initialization failed"

**Cause**: ROS2 environment not sourced.

**Fix**:
```bash
source /opt/ros/humble/setup.bash
export ROS_DOMAIN_ID=0
cargo run --release --features ros2
```

### "No topics found"

**Cause**: No publishers active on the ROS2 network.

**Fix**: Start publishers in another terminal:
```bash
ros2 run demo_nodes_cpp talker
ros2 run demo_nodes_py listener
```

### "Segment file empty"

**Cause**: Recording started but no messages received yet.

**Fix**: Wait a few seconds and check again:
```bash
sleep 5
ls -lh ./data/segment-*.log
```

### "Permission denied on ./data/"

**Cause**: Directory permissions issue.

**Fix**:
```bash
chmod 755 ./data/
rm -rf ./data/*
```

## Performance Benchmarks

After recording for 1 minute with 3 topics at 100 Hz each:

```
Expected results:
- Total messages: ~18,000 (3 topics × 100 Hz × 60s)
- Disk usage: ~5-10 MB (depending on message size)
- CPU: 15-30% (single core)
- Memory: ~100 MB
- WAL recovery time: <100 ms
```

## Next Steps

1. **Scale to multiple robots**: Run multiple recorders with different `ROS_DOMAIN_ID` values
2. **Implement cloud sync**: Point to real S3 bucket in `config/default.toml`
3. **Add encryption**: Enable AES-GCM in storage module
4. **Deploy to robot**: Copy binary to robot and set up systemd service

## Service Deployment (Optional)

Create `/etc/systemd/system/ros2-recorder.service`:

```ini
[Unit]
Description=ROS2 Recorder Service
After=network.target

[Service]
Type=simple
User=robot
WorkingDirectory=/home/robot/rust_ros2_recording_dashboard
ExecStart=/home/robot/rust_ros2_recording_dashboard/target/release/rust_ros2_recorder
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
Environment="ROS_DOMAIN_ID=0"
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable ros2-recorder
sudo systemctl start ros2-recorder
sudo journalctl -u ros2-recorder -f
```

---

Enjoy recording ROS2 data! 🤖
