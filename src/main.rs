// Linky - Modern C2 Framework using Windows crate
// This is a from-scratch rewrite using modern Rust and windows crate

use linky::c2::{C2Config, C2Server, Implant, ImplantStatus};
use linky::server::start_c2_server;
use linky::utils::{base64_encode, generate_implant_id};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    log::info!("Linky C2 Framework - Modern Rust Implementation");

    // Initialize C2 server with default configuration
    let c2_config = C2Config::default();
    let c2_server = Arc::new(C2Server::new(c2_config.clone()));

    log::info!("C2 Server initialized with config: {:?}", c2_server.config);

    // Create a sample implant
    let sample_implant = Implant {
        id: generate_implant_id(),
        hostname: "WORKSTATION-01".to_string(),
        username: "user".to_string(),
        ip_address: "192.168.1.100".to_string(),
        platform: "Windows 10".to_string(),
        last_checkin: chrono::Utc::now(),
        status: ImplantStatus::Active,
        tasks: Vec::new(),
    };

    // Add implant to server
    c2_server.add_implant(sample_implant.clone());

    log::info!("Added implant: {:?}", sample_implant);
    log::info!("Current implants: {:?}", c2_server.get_implants());

    // Demonstrate encryption
    let test_data = "Hello from Linky C2 Framework";
    let encrypted = base64_encode(test_data.as_bytes());
    log::info!("Encrypted data: {}", encrypted);

    println!("\n=== Linky C2 Framework ===");
    println!("✅ Framework initialized successfully");
    println!(
        "✅ C2 Server configured on {}:{}",
        c2_server.config.server_address, c2_server.config.port
    );
    println!("✅ Sample implant registered: {}", sample_implant.id);
    println!("✅ Encryption working: {} -> {}", test_data, encrypted);
    println!("\nStarting C2 server...");

    // Start the C2 server
    start_c2_server(c2_server.clone()).await?;

    Ok(())
}
