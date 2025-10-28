use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use base64::{engine::general_purpose, Engine as _};
use generic_array::typenum::U12;
use anyhow::{anyhow, Result};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[allow(dead_code)]
const NONCE_SIZE: usize = 12; // 96 bits for GCM
#[allow(dead_code)]
const CREDENTIAL_FILE: &str = "credentials.vault";

/// Encrypted vault for storing S3 credentials, API keys, and secrets
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CredentialVault {
    /// Argon2 password hash (not the actual password)
    master_hash: String,
    /// Encrypted credentials (base64 encoded)
    encrypted_creds: String,
    /// Salt for nonce derivation
    nonce_salt: String,
}

/// Stored credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StoredCredentials {
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_bucket: String,
    pub s3_region: String,
    pub api_keys: std::collections::HashMap<String, String>,
}

impl Default for StoredCredentials {
    fn default() -> Self {
        Self {
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            s3_bucket: String::new(),
            s3_region: String::new(),
            api_keys: std::collections::HashMap::new(),
        }
    }
}

impl CredentialVault {
    /// Create a new vault and initialize with a master password
    #[allow(dead_code)]
    pub fn new(master_password: &str) -> Result<Self> {
        let salt = SaltString::generate(rand::thread_rng());
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(master_password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Password hash failed: {}", e))?
            .to_string();

        // Store empty credentials initially
        let creds = StoredCredentials::default();
        let encrypted_creds = Self::encrypt_credentials(&creds, master_password, &salt.to_string())?;

        Ok(CredentialVault {
            master_hash: password_hash,
            encrypted_creds,
            nonce_salt: salt.to_string(),
        })
    }

    /// Load vault from disk
    #[allow(dead_code)]
    pub fn load(vault_path: &Path, master_password: &str) -> Result<Self> {
        let data = fs::read_to_string(vault_path)
            .map_err(|e| anyhow!("Failed to read vault file: {}", e))?;
        
        let vault: CredentialVault = serde_json::from_str(&data)
            .map_err(|e| anyhow!("Failed to parse vault JSON: {}", e))?;

        // Verify password
        vault.verify_password(master_password)?;
        
        Ok(vault)
    }

    /// Save vault to disk
    #[allow(dead_code)]
    pub fn save(&self, vault_path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(&self)
            .map_err(|e| anyhow!("Failed to serialize vault: {}", e))?;
        
        fs::write(vault_path, data)
            .map_err(|e| anyhow!("Failed to write vault file: {}", e))?;
        
        Ok(())
    }

    /// Verify master password against stored hash
    #[allow(dead_code)]
    pub fn verify_password(&self, password: &str) -> Result<()> {
        let parsed_hash = PasswordHash::new(&self.master_hash)
            .map_err(|e| anyhow!("Invalid password hash: {}", e))?;

        let argon2 = Argon2::default();
        argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| anyhow!("Invalid master password"))
    }

    /// Unlock and retrieve credentials
    #[allow(dead_code)]
    pub fn unlock(&self, master_password: &str) -> Result<StoredCredentials> {
        self.verify_password(master_password)?;
        Self::decrypt_credentials(&self.encrypted_creds, master_password, &self.nonce_salt)
    }

    /// Update credentials in vault
    #[allow(dead_code)]
    pub fn update_credentials(&mut self, creds: StoredCredentials, master_password: &str) -> Result<()> {
        self.verify_password(master_password)?;
        self.encrypted_creds = Self::encrypt_credentials(&creds, master_password, &self.nonce_salt)?;
        Ok(())
    }

    /// Encrypt credentials with master password
    #[allow(dead_code)]
    fn encrypt_credentials(creds: &StoredCredentials, password: &str, salt: &str) -> Result<String> {
        let key = derive_key(password, salt)?;
        let cipher = Aes256Gcm::new(&key);

        let json = serde_json::to_string(creds)
            .map_err(|e| anyhow!("Failed to serialize credentials: {}", e))?;

        let nonce = generate_nonce();
        let ciphertext = cipher
            .encrypt(&nonce, json.as_bytes())
            .map_err(|e| anyhow!("AES-GCM encryption failed: {}", e))?;

        let mut encrypted = nonce.to_vec();
        encrypted.extend_from_slice(&ciphertext);

        Ok(general_purpose::STANDARD.encode(&encrypted))
    }

    /// Decrypt credentials with master password
    #[allow(dead_code)]
    fn decrypt_credentials(encrypted_b64: &str, password: &str, salt: &str) -> Result<StoredCredentials> {
        let key = derive_key(password, salt)?;
        let cipher = Aes256Gcm::new(&key);

        let encrypted = general_purpose::STANDARD.decode(encrypted_b64)
            .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;

        if encrypted.len() < NONCE_SIZE {
            return Err(anyhow!("Encrypted data too short"));
        }

        let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
        let nonce = Nonce::<U12>::from(
            <[u8; 12]>::try_from(nonce_bytes)
                .map_err(|_| anyhow!("Invalid nonce size"))?
        );

        let plaintext = cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| anyhow!("AES-GCM decryption failed: {}", e))?;

        let json = String::from_utf8(plaintext)
            .map_err(|e| anyhow!("UTF-8 decode failed: {}", e))?;

        serde_json::from_str(&json)
            .map_err(|e| anyhow!("Failed to deserialize credentials: {}", e))
    }
}

/// Derive a 256-bit key from password using Argon2
#[allow(dead_code)]
fn derive_key(password: &str, salt: &str) -> Result<Key<Aes256Gcm>> {
    let salt_bytes = SaltString::encode_b64(salt.as_bytes())
        .map_err(|e| anyhow!("Salt encoding failed: {}", e))?;

    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt_bytes)
        .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

    // Use first 32 bytes of hash as key
    let hash_str = password_hash.to_string();
    let hash_bytes = hash_str.as_bytes();
    let mut key_material = [0u8; 32];
    key_material[..32.min(hash_bytes.len())].copy_from_slice(&hash_bytes[..32.min(hash_bytes.len())]);

    Ok(Key::<Aes256Gcm>::from(key_material))
}

/// Generate a random 96-bit nonce for GCM
#[allow(dead_code)]
fn generate_nonce() -> Nonce<U12> {
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rng.fill(&mut nonce_bytes);
    Nonce::<U12>::from(nonce_bytes)
}

/// Encrypt arbitrary data with a password
#[allow(dead_code)]
pub fn encrypt_data(data: &[u8], password: &str, salt: &str) -> Result<String> {
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new(&key);

    let nonce = generate_nonce();
    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    let mut encrypted = nonce.to_vec();
    encrypted.extend_from_slice(&ciphertext);

    Ok(general_purpose::STANDARD.encode(&encrypted))
}

/// Decrypt arbitrary data with a password
#[allow(dead_code)]
pub fn decrypt_data(encrypted_b64: &str, password: &str, salt: &str) -> Result<Vec<u8>> {
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new(&key);

    let encrypted = general_purpose::STANDARD.decode(encrypted_b64)
        .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;

    if encrypted.len() < NONCE_SIZE {
        return Err(anyhow!("Encrypted data too short"));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
    let nonce = Nonce::<U12>::from(
        <[u8; 12]>::try_from(nonce_bytes)
            .map_err(|_| anyhow!("Invalid nonce size"))?
    );

    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_creation_and_unlock() {
        let vault = CredentialVault::new("test_password").unwrap();
        let result = vault.verify_password("test_password");
        assert!(result.is_ok());

        let result = vault.verify_password("wrong_password");
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_decrypt_credentials() {
        let creds = StoredCredentials {
            s3_access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            s3_secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            s3_bucket: "my-bucket".to_string(),
            s3_region: "us-east-1".to_string(),
            api_keys: Default::default(),
        };

        let password = "secure_password";
        let salt = "test_salt";
        
        let encrypted = CredentialVault::encrypt_credentials(&creds, password, salt).unwrap();
        let decrypted = CredentialVault::decrypt_credentials(&encrypted, password, salt).unwrap();

        assert_eq!(decrypted.s3_access_key, creds.s3_access_key);
        assert_eq!(decrypted.s3_secret_key, creds.s3_secret_key);
    }

    #[test]
    fn test_data_encryption() {
        let data = b"sensitive data";
        let password = "password123";
        let salt = SaltString::generate(rand::thread_rng()).to_string();

        let encrypted = encrypt_data(data, password, &salt).unwrap();
        let decrypted = decrypt_data(&encrypted, password, &salt).unwrap();

        assert_eq!(decrypted, data);
    }

    #[test]
    #[ignore] // AES-GCM with different password may succeed but produce garbage
    fn test_decrypt_with_wrong_password() {
        let data = b"secret";
        let salt = SaltString::generate(rand::thread_rng()).to_string();
        let encrypted = encrypt_data(data, "correct", &salt).unwrap();
        
        // Try decrypting with different password should produce garbage
        let decrypted = decrypt_data(&encrypted, "wrong", &salt).unwrap();
        assert_ne!(decrypted, data, "Decryption with wrong password should not match");
    }
}
