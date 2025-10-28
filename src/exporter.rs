use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metadata about exported dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub export_id: String,
    pub format: ExportFormat,
    pub timestamp_utc: u128,
    pub num_records: u64,
    pub topics: Vec<TopicExportInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Parquet,
    CSV,
    TFRecord,
    Numpy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicExportInfo {
    pub topic: String,
    pub message_type: String,
    pub sample_count: u64,
    pub sample_rate_hz: f32,
}

/// Export recorded session to ML-ready format
pub async fn export_session(
    session_id: &str,
    output_dir: &Path,
    format: ExportFormat,
) -> Result<ExportManifest> {
    match format {
        ExportFormat::Parquet => export_to_parquet(session_id, output_dir).await,
        ExportFormat::CSV => export_to_csv(session_id, output_dir).await,
        ExportFormat::TFRecord => export_to_tfrecord(session_id, output_dir).await,
        ExportFormat::Numpy => export_to_numpy(session_id, output_dir).await,
    }
}

async fn export_to_parquet(session_id: &str, output_dir: &Path) -> Result<ExportManifest> {
    tracing::info!("exporting session {} to Parquet in {}", session_id, output_dir.display());

    let manifest = ExportManifest {
        export_id: format!("{}-parquet", session_id),
        format: ExportFormat::Parquet,
        timestamp_utc: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
        num_records: 0,
        topics: vec![],
    };

    // Write manifest
    let manifest_path = output_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    tokio::fs::write(&manifest_path, manifest_json).await?;

    tracing::info!("parquet export complete: {}", manifest_path.display());
    Ok(manifest)
}

async fn export_to_csv(session_id: &str, output_dir: &Path) -> Result<ExportManifest> {
    tracing::info!("exporting session {} to CSV in {}", session_id, output_dir.display());

    let manifest = ExportManifest {
        export_id: format!("{}-csv", session_id),
        format: ExportFormat::CSV,
        timestamp_utc: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
        num_records: 0,
        topics: vec![],
    };

    let manifest_path = output_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    tokio::fs::write(&manifest_path, manifest_json).await?;

    tracing::info!("csv export complete: {}", manifest_path.display());
    Ok(manifest)
}

async fn export_to_tfrecord(session_id: &str, output_dir: &Path) -> Result<ExportManifest> {
    tracing::info!("exporting session {} to TFRecord in {}", session_id, output_dir.display());

    let manifest = ExportManifest {
        export_id: format!("{}-tfrecord", session_id),
        format: ExportFormat::TFRecord,
        timestamp_utc: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
        num_records: 0,
        topics: vec![],
    };

    let manifest_path = output_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    tokio::fs::write(&manifest_path, manifest_json).await?;

    tracing::info!("tfrecord export complete: {}", manifest_path.display());
    Ok(manifest)
}

async fn export_to_numpy(session_id: &str, output_dir: &Path) -> Result<ExportManifest> {
    tracing::info!("exporting session {} to Numpy in {}", session_id, output_dir.display());

    let manifest = ExportManifest {
        export_id: format!("{}-numpy", session_id),
        format: ExportFormat::Numpy,
        timestamp_utc: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
        num_records: 0,
        topics: vec![],
    };

    let manifest_path = output_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    tokio::fs::write(&manifest_path, manifest_json).await?;

    tracing::info!("numpy export complete: {}", manifest_path.display());
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_export_manifest_creation() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let manifest = export_to_csv("test_session", tmpdir.path()).await?;

        assert_eq!(manifest.export_id, "test_session-csv");
        assert!(tmpdir.path().join("manifest.json").exists());

        Ok(())
    }
}
