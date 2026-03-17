mod cli;
mod generate;
mod links;
mod routes;
mod server;
mod tasks;

use std::sync::{Arc, Mutex};

use links::Links;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bind_addr = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "0.0.0.0:443".to_string());

    println!("╔══════════════════════════════╗");
    println!("║       Linky C2 Framework     ║");
    println!("╚══════════════════════════════╝");
    println!("[*] Starting HTTPS listener on {}\n", bind_addr);

    let links = Arc::new(Mutex::new(Links::default()));

    // Start HTTPS C2 server in its own OS thread (actix uses Rc internally → not Send)
    let links_srv = links.clone();
    let addr = bind_addr.clone();
    std::thread::spawn(move || {
        let sys = actix_web::rt::System::new();
        sys.block_on(async move {
            if let Err(e) = server::start(links_srv, &addr).await {
                eprintln!("[-] Server error: {}", e);
                std::process::exit(1);
            }
        });
    });

    // Background thread: mark stale links as inactive every 30 s
    let links_gc = links.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(30));
        links_gc.lock().unwrap().mark_inactive();
    });

    // CLI runs on the main thread (rustyline is synchronous)
    cli::run(links);
}
