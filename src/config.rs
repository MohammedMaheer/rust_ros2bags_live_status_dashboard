use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub path: PathBuf,
    pub wal_segment_size: usize,
    pub compress: bool,
    pub encryption: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncConfig {
    pub endpoint: String,
    pub bucket: Option<String>,
    pub chunk_size: usize,
    pub max_retries: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub storage: StorageConfig,
    pub sync: SyncConfig,
}

impl AppConfig {
    pub fn load_default() -> anyhow::Result<Self> {
        let default = include_str!("../config/default.toml");
        let cfg: AppConfig = toml::from_str(default)?;
        Ok(cfg)
    }

    pub fn load_from(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let p = path.into();
        let s = fs::read_to_string(&p)?;
        let cfg: AppConfig = toml::from_str(&s)?;
        Ok(cfg)
    }
}
