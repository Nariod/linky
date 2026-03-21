// Utilities Module

use base64::{engine::general_purpose, Engine as _};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn generate_implant_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("implant-{}-{}", timestamp, uuid::Uuid::new_v4().simple())
}

pub fn base64_encode(data: &[u8]) -> String {
    general_purpose::STANDARD.encode(data)
}

pub fn base64_decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
    general_purpose::STANDARD.decode(data)
}

pub fn encrypt_data(data: &[u8], key: &[u8]) -> Vec<u8> {
    // Simple XOR encryption for demonstration
    // In production, use proper encryption like AES
    data.iter()
        .zip(key.iter().cycle())
        .map(|(d, k)| d ^ k)
        .collect()
}

pub fn decrypt_data(data: &[u8], key: &[u8]) -> Vec<u8> {
    // XOR decryption (same as encryption for XOR)
    encrypt_data(data, key)
}
