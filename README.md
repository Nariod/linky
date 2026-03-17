# Linky - Modern C2 Framework

A modern Command and Control (C2) framework written in Rust, designed for security research and penetration testing. Linky provides a flexible architecture for managing implants across multiple platforms.

## Features

- **Cross-platform implants**: Windows, Linux, and macOS support
- **Secure communication**: Encrypted C2 messaging with base64 encoding
- **Task management**: Execute commands and receive results from implants
- **Implant tracking**: Monitor implant status, check-ins, and system information
- **Modular architecture**: Easy to extend with new features
- **REST API**: HTTP-based communication for implant-server interaction

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Linky C2 Framework                      │
├─────────────────┬─────────────────┬─────────────────┬───────┤
│   C2 Server     │   Implants      │   Task System   │  API  │
│  (Actix Web)    │  (Cross-platform)│ (Async)        │ (REST)│
└─────────────────┴─────────────────┴─────────────────┴───────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│                     Communication Flow                      │
└─────────────────────────────────────────────────────────────┘

1. Implant registration with system information
2. Periodic check-ins for status updates
3. Task retrieval and execution
4. Result submission back to C2 server
```

## Quick Start

### Prerequisites

- Rust 1.70+ (with Cargo)
- OpenSSL development libraries (for HTTPS support)
- Actix-web dependencies

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/linky.git
cd linky

# Build the project
cargo build --release

# Run the C2 server
cargo run --release
```

### Running the Demo

```bash
# Run the simple demo to see framework functionality
cargo run --example simple_demo
```

## Project Structure

```
linky/
├── Cargo.toml              # Workspace configuration
├── src/                    # Core framework
│   ├── c2/                 # C2 server and implant management
│   ├── server/             # HTTP server implementation
│   ├── utils/              # Utility functions
│   ├── implants/           # Implant generation
│   └── lib.rs              # Main library exports
├── links/                  # Platform-specific implants
│   ├── windows/            # Windows implant
│   ├── linux/              # Linux implant
│   └── osx/                # macOS implant
├── examples/               # Example code and demos
└── README.md               # This file
```

## Core Components

### C2 Server

The central command and control server that:
- Manages implant connections
- Distributes tasks to implants
- Collects and stores results
- Provides REST API endpoints for implant communication

### Implants

Lightweight agents that run on target systems:
- **Windows**: Full-featured implant with process injection capabilities
- **Linux**: Basic command execution and file operations
- **macOS**: Similar functionality to Linux implant

### Task System

- **Task creation**: Operators can assign commands to specific implants
- **Status tracking**: Monitor task progress (Pending, InProgress, Completed, Failed)
- **Result collection**: Store and retrieve command output

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/register` | POST | Implant registration |
| `/api/checkin` | POST | Implant check-in |
| `/api/task` | POST | Get pending tasks |
| `/api/result` | POST | Submit task results |
| `/api/status` | GET | Server status |

## Configuration

The C2 server uses a default configuration that can be customized:

```rust
C2Config {
    server_address: "0.0.0.0",
    port: 8443,
    use_https: true,
    encryption_key: "default-encryption-key-12345",
    ssl_cert_path: None,
    ssl_key_path: None,
}
```

## Security Features

- **Message encryption**: XOR-based encryption with configurable keys
- **Base64 encoding**: All messages are base64 encoded for safe transmission
- **HTTPS support**: Optional SSL/TLS encryption for all communications
- **Authentication**: Implant registration with unique identifiers

## Building Implants

The framework includes tools to generate platform-specific implants:

```rust
// Generate a Windows implant
generate_windows_implant("output.cs", "c2-server-address.com")?;

// Generate a Linux implant
generate_linux_implant("output.sh", "c2-server-address.com")?;

// Generate a macOS implant
generate_mac_implant("output.sh", "c2-server-address.com")?;
```

## Windows Implant Features

The Windows implant includes advanced capabilities:

- **Process injection**: Inject shellcode into remote processes
- **Command execution**: Run commands through cmd.exe or PowerShell
- **File operations**: List directories, change working directory
- **System information**: Retrieve username, hostname, IP address
- **Integrity level**: Check process privilege level

## Usage Example

```rust
use linky::c2::{C2Config, C2Server, Implant, ImplantStatus};
use linky::utils::generate_implant_id;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize C2 server
    let config = C2Config::default();
    let c2_server = C2Server::new(config);

    // Create and register an implant
    let implant = Implant {
        id: generate_implant_id(),
        hostname: "WORKSTATION-01".to_string(),
        username: "user".to_string(),
        ip_address: "192.168.1.100".to_string(),
        platform: "Windows 10".to_string(),
        last_checkin: chrono::Utc::now(),
        status: ImplantStatus::Active,
        tasks: Vec::new(),
    };

    c2_server.add_implant(implant);

    // Add a task to the implant
    c2_server.add_task(&implant.id, "whoami");

    Ok(())
}
```

## Development

### Running Tests

```bash
cargo test
```

### Building for Release

```bash
cargo build --release
```

### Cross-compilation

To build Windows implants from Linux:

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu --release
```

## License

This project is licensed for security research and educational purposes only. Do not use for unauthorized or illegal activities.

## Disclaimer

This tool is provided for educational and research purposes only. The developers are not responsible for any misuse or damage caused by this software. Always obtain proper authorization before testing on any system.

## Contributing

Contributions are welcome! Please open issues for bugs or feature requests, and submit pull requests for improvements.

## Roadmap

- [ ] Advanced encryption options (AES, ChaCha20)
- [ ] Persistence mechanisms for implants
- [ ] File transfer capabilities
- [ ] Screenshot functionality
- [ ] Keylogging features
- [ ] Web interface for C2 management
- [ ] Plugin system for extensibility
