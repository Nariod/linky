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
| **Small codebase** | ~3k LOC vs ~50k for Sliver — auditable, forkable |
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
│   ├── common/             # Shared: HTTP client, encryption, wire types
│   ├── linux/              # Linux implant    → link-linux
│   ├── windows/            # Windows implant  → link-windows.exe
│   └── osx/                # macOS implant    → link-osx (stub)
├── Dockerfile              # Single-stage with full Rust toolchain
├── CLAUDE.md               # Instructions for Claude Code
└── .mistralrc              # Instructions for Mistral Vibe
```

### C2 protocol (3 stages)

All communication is HTTPS/JSON. Each request is validated against a fixed User-Agent, a session cookie, and a rolling UUID header.

```
Implant                           Server
  │                                  │
  │── GET /js ───────────────────────▶│  Stage 1: Set-Cookie
  │                                  │
  │── POST /static/register ─────────▶│  Stage 2: register + initial x_request_id
  │◀─ { x_request_id } ──────────────│
  │                                  │
  │── POST /static/get ──────────────▶│  Stage 3: polling loop (configurable)
  │   header: x-request-id           │    → server returns next task
  │   body: { q, tasking }           │    ← implant submits task output
  │◀─ { q, tasking, x_request_id } ──│
```

---

## CLI reference

### Main menu

```
linky> help
  links                    Manage active links
  generate <ip:port>       Build Windows implant (x86_64-pc-windows-gnu)
  generate-linux <ip:port> Build Linux implant   (x86_64-unknown-linux-musl)
  generate-osx <ip:port>   Build macOS implant   (x86_64-apple-darwin)
  help                     Show this help
  exit / kill              Quit linky
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
  ── Windows only ─────────────────────────────────────
  integrity                Token integrity level
  inject <pid> <b64>       Shellcode injection into PID
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

### Docker/Podman

```bash
podman build -t linky-c2 .
podman run -it --rm -p 8443:8443 -v ./implants:/implants:Z linky-c2
```

---

## Implant capabilities

| Feature | Linux | Windows | macOS |
|---------|-------|---------|-------|
| Shell execution | `/bin/sh -c` | `cmd.exe /C` (CREATE_NO_WINDOW) | `/bin/sh -c` |
| System info | `/proc`, `/etc/os-release` | PowerShell, env vars | `sw_vers`, `scutil` |
| Process listing | `/proc` parsing | `tasklist /FO CSV` | — |
| Network connections | `/proc/net/tcp*` | `netstat -ano` | — |
| File download/upload | ✅ | ✅ | — |
| Configurable sleep+jitter | ✅ | ✅ | — |
| Kill date | ✅ | ✅ | — |
| Encrypted config | AES-256-GCM | AES-256-GCM | — |
| Shellcode injection | — | VirtualAllocEx + CreateRemoteThread | — |
| Integrity level | — | Token query | — |

---

## Roadmap

### Current limitations (honest assessment)

- **Crypto**: encryption key is shared across all implants (hardcoded derivation). No payload encryption on the wire (JSON in cleartext over self-signed TLS).
- **Evasion**: zero EDR evasion. Windows injection uses the most hooked APIs. No AMSI/ETW bypass. Static strings (UA, cookie, routes) are trivial signatures.
- **Features**: no persistence, no SOCKS proxy, no credential harvesting, no lateral movement.
- **Operations**: single operator, no logging, no database, no web UI.
- **macOS**: stub only — no encryption, no jitter, no file ops.

### MVP roadmap (target: ~50% feature coverage)

| Sprint | Focus | Duration |
|--------|-------|----------|
| 0 | Dead code cleanup, error handling | ~3 days |
| 1 | Per-implant keys, payload encryption, string obfuscation | ~2 weeks |
| 2 | Malleable profiles, binary size, indirect syscalls, AMSI/ETW | ~2 weeks |
| 3 | Persistence (Linux+Windows), SOCKS proxy, op logging | ~3 weeks |
| 4 | Integration tests, CI hardening, macOS alignment | ~1 week |

See `TODO.txt` for the detailed task list.

---

## Security notice

This tool is for **authorized** penetration testing engagements only. Do not use it against systems without explicit written permission.

## License

See LICENSE file.