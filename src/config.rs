use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub wal_segment_size: usize,
    #[allow(dead_code)]
    pub compress: bool,
    #[allow(dead_code)]
    pub encryption: Option<String>,
    #[serde(default = "default_encryption_enabled")]
    #[allow(dead_code)]
    pub enable_aes_gcm: bool,
}

fn default_encryption_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncConfig {
    #[allow(dead_code)]
    pub endpoint: String,
    #[allow(dead_code)]
    pub bucket: Option<String>,
    pub chunk_size: usize,
    #[allow(dead_code)]
    pub max_retries: usize,
    #[serde(default = "default_use_vault")]
    #[allow(dead_code)]
    pub use_credential_vault: bool,
    #[allow(dead_code)]
    pub vault_path: Option<PathBuf>,
}

fn default_use_vault() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_encryption_enabled")]
    #[allow(dead_code)]
    pub enable_encryption: bool,
    #[allow(dead_code)]
    pub vault_password_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub storage: StorageConfig,
    pub sync: SyncConfig,
    #[allow(dead_code)]
    pub security: Option<SecurityConfig>,
}

impl AppConfig {
    pub fn load_default() -> anyhow::Result<Self> {
        let default = include_str!("../config/default.toml");
        let cfg: AppConfig = toml::from_str(default)?;
        Ok(cfg)
    }

    #[allow(dead_code)]
    pub fn load_from(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let p = path.into();
        let s = fs::read_to_string(&p)?;
        let cfg: AppConfig = toml::from_str(&s)?;
        Ok(cfg)
    }

    /// Get vault password from environment or prompt
    #[allow(dead_code)]
    pub fn get_vault_password(&self) -> anyhow::Result<Option<String>> {
        if let Some(security) = &self.security {
            if let Some(env_var) = &security.vault_password_env {
                return Ok(std::env::var(env_var).ok());
            }
        }
        Ok(None)
    }
}

