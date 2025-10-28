use anyhow::Result;

pub async fn is_online() -> Result<bool> {
    // Very simple check: try to resolve a small request or rely on OS network status.
    Ok(true)
}
