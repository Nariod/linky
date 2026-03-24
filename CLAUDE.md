# CLAUDE.md — Instructions pour Claude Code

## Projet

Linky est un framework C2 (Command & Control) écrit en Rust idiomatique.
C'est une réécriture de [postrequest/link](https://github.com/postrequest/link).

**Philosophie : KISS + MVP.** Chaque modification doit être la solution la plus
simple qui fonctionne. Pas de sur-ingénierie, pas d'abstractions prématurées.

## Architecture

```
linky/                          ← workspace root (Cargo.toml resolver = "2")
├── server/                     ← C2 server (crate: linky)
│   └── src/
│       ├── main.rs             ← Entry point : 3 threads (server, GC, CLI)
│       ├── server.rs           ← actix-web 4 + rustls 0.23 (self-signed via rcgen)
│       ├── routes.rs           ← HTTP handlers : protocole C2 3-stage
│       ├── links.rs            ← Registre des links + state management
│       ├── tasks.rs            ← File de tâches par link
│       ├── cli.rs              ← CLI interactif (rustyline)
│       ├── generate.rs         ← Générateur d'implants (cargo build)
│       └── ui.rs               ← Helpers d'affichage terminal
├── links/
│   ├── common/                 ← Crate partagé (crate: link-common)
│   │   └── src/lib.rs          ← Types wire, HTTP client, crypto
│   ├── linux/                  ← Implant Linux (crate: link-linux)
│   ├── windows/                ← Implant Windows (crate: link-windows)
│   └── osx/                    ← Implant macOS (crate: link-osx) — stub
├── Dockerfile                  ← Build conteneurisé
├── TODO.txt                    ← Roadmap détaillé (lire en premier)
└── CLAUDE.md                   ← Ce fichier
```

## Convention Rust

- **Édition** : Rust 2021
- **Style** : `cargo fmt --all` (rustfmt par défaut)
- **Lint** : `cargo clippy --workspace -- -D warnings` (zéro warning)
- **Nommage** : snake_case fonctions/variables, PascalCase types/traits, SCREAMING_SNAKE_CASE constantes
- **Erreurs** : `thiserror` pour les types d'erreur, `anyhow` pour la propagation rapide. Pas de `.unwrap()` en dehors des tests.
- **Strings** : Préférer `&str` à `String` quand possible
- **Itérateurs** : Préférer les chaînes d'itérateurs aux boucles for
- **Unsafe** : Interdit sauf justification documentée (ex: FFI Windows). Chaque bloc `unsafe` doit avoir un commentaire `// SAFETY:`.
- **Deps** : Minimiser les dépendances. Préférer la stdlib quand c'est raisonnable.

## Commandes courantes

```bash
# Build
cargo build --release -p linky              # Serveur
cargo build --release -p link-linux         # Implant Linux (nécessite CALLBACK env)
CALLBACK=127.0.0.1:8443 cargo build -p link-linux  # Avec callback

# Qualité
cargo fmt --all -- --check                  # Format check
cargo clippy --workspace -- -D warnings     # Lint
cargo test --workspace                      # Tous les tests
cargo audit                                 # Vulnérabilités deps

# Cross-compilation
cargo build --release --target x86_64-unknown-linux-musl -p link-linux
cargo build --release --target x86_64-pc-windows-gnu -p link-windows

# Conteneur
podman build -t linky-c2 .
podman run -it --rm -p 8443:8443 -v ./implants:/implants:Z linky-c2
```

## Workflow pour les modifications

1. **Lire TODO.txt** avant toute modification pour comprendre les priorités.
2. **Plan mode** : toujours planifier avant de coder. Écrire le plan en commentaire.
3. **Tester** : après chaque modification, exécuter `cargo clippy --workspace -- -D warnings && cargo test --workspace`.
4. **Un commit = une chose** : pas de commits mélangeant refactor et feature.
5. **Context management** : utiliser `/compact` ou `/clear` quand le contexte devient trop grand.

## Règles spécifiques au projet

### Sécurité
- **Jamais** de clé ou secret hardcodé en production. Les clés de test doivent être dans `#[cfg(test)]`.
- **Zeroize** les buffers contenant des clés après usage.
- **Pas de log** des secrets, clés, ou payloads déchiffrés.
- Les strings statiques sensibles (UA, cookie, routes) doivent être obfusquées dans les implants.

### Implants
- Le code des implants doit être **minimal** — chaque octet compte.
- Préférer `std::fs::read()` / `std::fs::write()` aux patterns File::open + read.
- Les fonctions partagées entre implants vont dans `links/common/`.
- L'implant macOS (`links/osx/`) est un stub — l'aligner sur Linux/Windows progressivement.
- Les callbacks réseau utilisent toujours `danger_accept_invalid_certs(true)` (self-signed TLS).

### Serveur
- L'état global est dans `Arc<Mutex<Links>>` — acquérir le lock le moins longtemps possible.
- **Pas de lock imbriqué** — drop(guard) explicitement avant de re-locker (risque de deadlock).
- Les messages UI passent par `ui::print*()`, pas par `println!()` direct.
- Le logging technique passe par `tracing::info!()` / `tracing::error!()`.

### Tests
- Tests unitaires dans le même fichier (`#[cfg(test)] mod tests`).
- Tests d'intégration dans `server/tests/`.
- Nommer les tests `test_<what>_<expected_behavior>`.

## Protocole C2 — Référence rapide

```
Stage 1: GET  /js              → Set-Cookie: banner=banner
Stage 2: POST /static/register → body: RegisterRequest → resp: TaskResponse
Stage 3: POST /static/get      → header: x-request-id + body: CallbackRequest
                                → resp: TaskResponse (command + new x_request_id)
```

Validation à chaque stage :
- User-Agent == constante IMPLANT_UA
- Cookie contient "banner=banner"
- x-request-id valide (stage 3)

## Crates clés utilisés

| Crate | Usage | Version |
|-------|-------|---------|
| actix-web | Serveur HTTP | 4 |
| rustls | TLS | 0.23 |
| rcgen | Génération certificat self-signed | 0.14 |
| reqwest | Client HTTP (implants, blocking) | 0.13 |
| aes-gcm | Chiffrement config | 0.10 |
| serde/serde_json | Sérialisation JSON | 1 |
| rustyline | CLI interactif | 17 |
| chrono | Dates/timestamps | 0.4 |
| tracing | Logging structuré | 0.1 |
| colored | Couleurs terminal | 3 |

## Bugs connus

- **File upload** : les implants ignorent `task.upload` / `task.upload_path` — le contenu n'est pas transmis.
- **Linux get_interface_ip()** : parsing de `/proc/net/fib_trie` fragile.
- **macOS** : stub sans encryption, jitter, file ops, ou kill date.