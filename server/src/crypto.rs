//! Server-side crypto primitives — shared between routes.rs and generate.rs.
//!
//! Must stay byte-for-byte compatible with link-common::derive_key /
//! encrypt_config / decrypt_config (same SHA-256 KDF, same AES-256-GCM layout).

/// Derive a 32-byte key from `secret` and `salt` using SHA-256.
pub fn derive_key(secret: &[u8], salt: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(salt.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result[..32]);
    key
}

/// Encrypt `data` with AES-256-GCM; return `hex(nonce || ciphertext)`.
pub fn encrypt(data: &str, key: &[u8; 32]) -> String {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key).expect("Failed to create cipher");
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .expect("Encryption failure");

    let mut result = Vec::with_capacity(nonce.len() + ciphertext.len());
    result.extend_from_slice(nonce);
    result.extend_from_slice(&ciphertext);
    hex::encode(result)
}

/// Decrypt a `hex(nonce || ciphertext)` blob; return the plaintext or `None` on failure.
pub fn decrypt(enc_hex: &str, key: &[u8; 32]) -> Option<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    let encrypted_data = hex::decode(enc_hex).ok()?;
    if encrypted_data.len() < 12 {
        return None;
    }
    let nonce = Nonce::from_slice(&encrypted_data[..12]);
    let ciphertext = &encrypted_data[12..];
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    match cipher.decrypt(nonce, ciphertext) {
        Ok(decrypted) => String::from_utf8(decrypted).ok(),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let k1 = derive_key(b"secret", "salt");
        let k2 = derive_key(b"secret", "salt");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_key(b"test-secret", "test-salt");
        let plaintext = "hello world";
        let encrypted = encrypt(plaintext, &key);
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_hex_returns_none() {
        let key = derive_key(b"test-secret", "test-salt");
        assert!(decrypt("not-hex!", &key).is_none());
    }

    #[test]
    fn test_decrypt_too_short_returns_none() {
        let key = derive_key(b"test-secret", "test-salt");
        assert!(decrypt("aabbccdd1122334455ff", &key).is_none());
    }

    #[test]
    fn test_wrong_key_returns_none() {
        let key1 = derive_key(b"key1", "salt");
        let key2 = derive_key(b"key2", "salt");
        let encrypted = encrypt("hello", &key1);
        assert!(decrypt(&encrypted, &key2).is_none());
    }
}
