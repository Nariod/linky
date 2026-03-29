# Linky C2 Framework

A minimal, Rust-native Command & Control framework. Rewrite of [postrequest/link](https://github.com/postrequest/link).

> **Status: Alpha — not production-ready.**
> See [Roadmap](#roadmap) for what's missing before real-world use.

> **SECURITY WARNING**
> This project was developed with assistance from Claude and Mistral AI.
> Always review and audit the code before use in security-sensitive environments.
> The authors accept no responsibility for unauthorized or illegal use.

---

## Quick start (Podman, 3 steps)
```bash
# 1. Build (includes full Rust toolchain for on-the-fly implant generation)
podman build -t linky-c2 .

# 2. Run (port 8443, implants volume — :Z required on SELinux systems)
podman run -it --rm \
  -p 8443:8443 \
  -v ./implants:/implants:Z \
  --name linky-server \
  linky-c2

# 3. Generate a Linux implant from the Linky CLI prompt
linky> generate-linux 192.168.1.10:8443
```

Your implant is in `./implants/link-linux`. No Rust installation needed on the host.

---

## Why Linky?

| Trait | Detail |
|-------|--------|
| **Rust-native** | Harder to reverse than Go/Python — limited RE tooling for Rust binaries |
| **Small codebase** | ~2k LOC vs ~50k for Sliver — auditable, forkable |
| **Container-first** | Build + run in 3 commands, no host dependencies |
| **KISS** | One binary, one protocol, no plugin system to learn |

Linky does **not** aim for feature parity with Sliver, Mythic, or Havoc. It's a focused, minimal C2 for simple engagements and learning.

---

## Architecture
```
linky/
├── server/                 # C2 server binary
│   └── src/
│       ├── main.rs         # Entry: server + GC + CLI threads
│       ├── server.rs       # actix-web 4 + rustls 0.23 (self-signed TLS)
│       ├── routes.rs       # HTTP handlers: 3-stage C2 protocol
│       ├── links.rs        # Link registry and state management
│       ├── tasks.rs        # Per-link task queue
│       ├── cli.rs          # Interactive CLI (rustyline)
│       ├── generate.rs     # Implant builder (invokes cargo)
│       └── ui.rs           # Terminal output helpers
├── links/
│   ├── common/             # Shared: C2 loop, HTTP client, crypto, wire types
│   │   └── src/
│   │       ├── lib.rs      # run_c2_loop(), crypto, types, helpers
│   │       └── dispatch.rs # Cross-platform command dispatch
│   ├── linux/              # Linux implant    → link-linux (~80 LOC)
│   ├── windows/            # Windows implant  → link-windows.exe (~120 LOC)
│   └── osx/                # macOS implant    → link-osx (~80 LOC)
├── podmanfile              # Single-stage with full Rust toolchain
├── CLAUDE.md               # Instructions for Claude Code
└── .mistralrc              # Instructions for Mistral Vibe
```

### C2 protocol (3 stages)

All communication is HTTPS/JSON with AES-256-GCM payload encryption. Each request is validated against an obfuscated User-Agent, a session cookie, and a rolling UUID header.
```
Implant                           Server
  │                                  │
  │── GET /js ───────────────────────▶│  Stage 1: Set-Cookie
  │                                  │
  │── POST /static/register ─────────▶│  Stage 2: register + initial x_request_id
  │◀─ { x_request_id, data? } ───────│
  │                                  │
  │── POST /static/get ──────────────▶│  Stage 3: polling loop
  │   header: x-request-id           │    body: AES-256-GCM encrypted payload
  │   body: { data: encrypted }      │    ← implant submits task output (encrypted)
  │◀─ { data: encrypted, x_req_id } ─│    → server returns next task (encrypted)
```

Each implant has a **unique AES-256-GCM key** derived from a per-implant secret generated at build time. Keys never appear in plaintext in binaries.

---

## CLI reference

### Main menu
```
linky> help
  links                                  Manage active links
  generate <ip:port> [--shellcode]       Build Windows implant (x86_64-pc-windows-gnu)
  generate-linux <ip:port> [--shellcode] Build Linux implant   (x86_64-unknown-linux-musl)
  generate-osx <ip:port> [--shellcode]   Build macOS implant   (x86_64-apple-darwin)
  help                                   Show this help
  exit / kill                            Quit linky

  --shellcode   Produce minimal .bin via objcopy (Linux/macOS).
                Windows: produces a PE — use sRDI/Donut for PIC conversion.
                Uses release-shellcode profile (panic=abort, lto, opt-level=z).

  LINKY_OUTPUT_DIR  Output directory for generated implants (default: .)
```

### Link interaction
```
  ── Execution ────────────────────────────────────────
  shell <cmd>              Raw command via /bin/sh or cmd.exe
  cmd <args>               cmd.exe /C wrapper         (Windows)
  powershell <args>        powershell.exe wrapper      (Windows)
  ── Navigation ───────────────────────────────────────
  ls / cd / pwd / whoami / pid
  ── Reconnaissance ───────────────────────────────────
  info                     System information
  ps                       Running processes
  netstat                  Network connections
  ── File transfer ────────────────────────────────────
  download <path>          Download file from implant
  upload <local> <remote>  Upload file to implant
  ── Operational ──────────────────────────────────────
  sleep <s> [jitter%]      Polling interval (e.g. sleep 30 20)
  killdate <date|clear>    Auto-exit date (e.g. killdate 2026-12-31)
  ── Windows ──────────────────────────────────────────
  integrity                Token integrity level
  inject <pid> <b64>       Inject base64 shellcode into PID
  ── Session ──────────────────────────────────────────
  kill                     Send exit + mark link dead
  back                     Return to links menu
```

---

## Building from source

### Prerequisites

**Server only:** Rust 1.70+

**Full (server + implant generation):**
```bash
# Debian/Ubuntu
sudo apt-get install -y musl-tools mingw-w64 clang lld pkg-config libssl-dev
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl

# Fedora/RHEL
sudo dnf install -y musl-gcc mingw64-gcc clang lld pkg-config openssl-devel
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl
```

### Build & run
```bash
cargo build --release -p linky
./target/release/linky 0.0.0.0:8443
```

### Podman
```bash
podman build -t linky-c2 .
podman run -it --rm -p 8443:8443 -v ./implants:/implants:Z linky-c2
```

---

## Implant capabilities

| Feature | Linux | Windows | macOS |
|---------|-------|---------|-------|
| Shell execution | `/bin/sh -c` | `cmd.exe /C` (CREATE_NO_WINDOW) | `/bin/sh -c` |
| System info | `/proc`, `/etc/os-release` | PowerShell, env vars | — (shell fallback) |
| Process listing | `/proc` parsing | `tasklist /FO CSV` | — (shell fallback) |
| Network connections | `/proc/net/tcp*` | `netstat -ano` | — (shell fallback) |
| File download/upload | ✅ | ✅ | ✅ |
| Configurable sleep+jitter | ✅ | ✅ | ✅ |
| Kill date | ✅ | ✅ | ✅ |
| Encrypted C2 comms | AES-256-GCM | AES-256-GCM | AES-256-GCM |
| Per-implant key | ✅ | ✅ | ✅ |
| Obfuscated strings | ✅ | ✅ | ✅ |
| Shellcode injection | — | VirtualAllocEx + CreateRemoteThread | — |
| Integrity level | — | Token query | — |

> macOS `info`, `ps`, `netstat` fall back to shell execution. Native implementations pending (item 4.4).

---

## Roadmap

### Current limitations (honest assessment)

- **Evasion**: zero EDR evasion. Windows injection uses the most heavily monitored APIs (VirtualAllocEx + CreateRemoteThread). No AMSI/ETW bypass. Binary signatures are unaddressed.
- **Features**: no persistence, no SOCKS proxy, no credential harvesting, no lateral movement.
- **Operations**: single operator, no logging to disk, no database, no web UI.
- **macOS**: `info`, `ps`, `netstat` not natively implemented — fall back to shell.
- **Transport**: TLS is self-signed, no certificate pinning or domain fronting.

### MVP roadmap (target: ~50% feature coverage)

| Sprint | Focus | Status |
|--------|-------|--------|
| 0 | Dead code cleanup, error handling | ✅ Done |
| 0.5 | Robustness, factorization debt | ✅ Done |
| 1 | Per-implant keys, payload encryption, string obfuscation | ✅ Done |
| 1.5 | Code factorization (run_c2_loop), mutex hardening | ✅ Done |
| 1.6 | Cargo.toml cleanup, CLAUDE.md update, CLI UX | ✅ Done |
| 2.2 | Build profiles (release + release-shellcode), implant size | ✅ Done |
| 2.6 | `--shellcode` flag: `.bin` via objcopy (Linux), PE (Windows) | ✅ Done |
| 2.x | Malleable profiles, indirect syscalls, AMSI/ETW bypass | ⬜ Planned |
| 3 | Persistence (Linux+Windows), SOCKS proxy, op logging | ⬜ Planned |
| 4 | Integration tests, CI hardening, macOS alignment | ⬜ Planned |

See `TODO.txt` for the detailed task list.

---

## Security notice

This tool is for **authorized** penetration testing engagements only. Do not use it against systems without explicit written permission.

---

## Testing
```bash
# Full quality check
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

# Build all implants (requires cross-compilation toolchain)
cargo check -p link-linux
cargo check -p link-windows
cargo check -p link-osx
cargo check -p link-common

# Run the server
cargo run --release --bin linky 0.0.0.0:8443
```

### Verify end-to-end
```
linky> generate-linux 127.0.0.1:8443
# Run the generated implant, then:
linky> links
linky> -i link-1
link-1> whoami
```

Result appears in a formatted box:
```
╔═ link-1 · whoami · 14:38:23 ═══════════════════╗
║ fedora@hostname
╚═════════════════════════════════════════════════╝
```

---

## Configuration for AI Developers

- **`CLAUDE.md`** — conventions, architecture, security rules for Claude Code
- **`.mistralrc`** — conventions and workflow for Mistral Vibe
- **`TODO.txt`** — authoritative roadmap (read before modifying anything)
