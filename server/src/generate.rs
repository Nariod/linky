use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

fn output_dir() -> PathBuf {
    std::env::var("LINKY_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
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

pub fn generate_native(callback: &str) {
    build(
        callback,
        "links/linux",
        "x86_64-unknown-linux-gnu",
        "link-linux-native",
    );
}

pub fn generate_osx(callback: &str) {
    build(
        callback,
        "links/osx",
        "x86_64-apple-darwin",
        "link-osx",
    );
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

    tracing::info!(
        "Building {} implant ({}) for {} …",
        output_name, target, callback
    );

    let result = Command::new("cargo")
        .env("CALLBACK", callback)
        .args(["build", "--release", "--target", target, "--quiet"])
        .current_dir(dir)
        .status();

    // Fix: Look for binary in workspace target directory
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
                tracing::error!(
                    "Build succeeded but binary not found at {}",
                    src.display()
                );
            }
        }
        Ok(s) => tracing::error!("Build failed (exit {})", s),
        Err(e) => tracing::error!("Failed to invoke cargo: {}", e),
    }
}
