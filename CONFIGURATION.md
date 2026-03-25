# Configuration et Tests Automatiques pour Linky C2

Ce document explique la structure de configuration et comment exécuter les tests automatiquement.

## 📁 Fichiers de Configuration

### Pour Mistral AI
- **`.mistralrc`** - Configuration principale pour les IA Mistral
  - Contient les commandes de test automatiques
  - Documentation des fichiers clés
  - Workflow de développement
  - Conventions de codage

### Pour Claude AI
- **`.claude/settings.local.json`** - Configuration pour Claude
  - Format JSON structuré
  - Commandes de test automatiques
  - Documentation des fonctionnalités
  - Conseils de débogage

## 🚀 Exécution des Tests

### Méthode 1: Utiliser Makefile (recommandé)

```bash
# Tous les tests
tmake test-full

# Tests spécifiques
make build      # Build en release
make test       # Tests unitaires
make check      # Vérifications (check + clippy)
make podman     # Build podman
make clean      # Nettoyage
```

### Méthode 2: Script complet

```bash
# Exécuter tous les tests
t./test_linky.sh
```

### Méthode 3: Commandes manuelles

```bash
# Vérification de base
cargo check
cargo clippy

# Build
cargo build --release

# Tests
cargo test

# podman
podman build -t linky-c2 .
```

## 📋 Structure du Projet

```
linky/
├── .mistralrc              # Config Mistral AI
├── .claude/                # Config Claude AI
│   └── settings.local.json
├── Makefile                # Commandes rapides
├── test_linky.sh           # Script de test complet
├── Cargo.toml              # Configuration Rust
├── podmanfile              # Configuration podman
├── server/                 # Code serveur principal
│   ├── src/
│   │   ├── cli.rs          # Interface CLI (affichage résultats)
│   │   ├── routes.rs       # Gestion requêtes HTTP
│   │   ├── links.rs        # Gestion implants/tâches
│   │   ├── tasks.rs        # Définition tâches
│   │   ├── main.rs         # Point d'entrée
│   │   └── ...
├── links/                  # Code implants
│   ├── common/
│   ├── windows/
│   ├── linux/
│   └── osx/
└── ...
```

## 🔍 Fichiers Clés à Comprendre

1. **`server/src/cli.rs`**
   - Interface CLI principale
   - Fonction `show_completed_task_results()` pour afficher les résultats
   - Gestion de l'interaction utilisateur

2. **`server/src/routes.rs`**
   - Gestion des requêtes HTTP des implants
   - Logique de complétion des tâches (`complete_task`)
   - Communication avec les implants

3. **`server/src/links.rs`**
   - Gestion des implants (links)
   - Système de tâches
   - Statut des implants

4. **`server/src/tasks.rs`**
   - Définition des tâches
   - Statuts (Waiting, InProgress, Completed)
   - Structure des données

## 🧪 Testing de la Fonctionnalité Principale

Pour tester l'affichage des résultats de commandes :

1. **Démarrer le serveur**
   ```bash
   cargo run --release --bin linky
   ```

2. **Générer un implant**
   ```
   generate-linux 127.0.0.1:8443
   ```

3. **Exécuter des commandes**
   ```
   whoami
   pwd
   info
   ```

4. **Vérifier l'affichage**
   - Les résultats devraient s'afficher dans des boîtes formatées
   - Exemple:
   ```
   ╔═ link-1 · whoami · 14:38:23 ═════════════════════════════╗
   ║ fedora@hostname
   ╚═══════════════════════════════════════════════════════════════╝
   ```

## 🐛 Problèmes Courants et Solutions

| Problème | Solution |
|----------|----------|
| Les résultats ne s'affichent pas | Vérifier `show_completed_task_results()` dans cli.rs |
| Erreurs de chemins de modules | Utiliser `crate::` au lieu des chemins relatifs |
| Problèmes de communication implant | Vérifier les logs dans routes.rs autour de `complete_task()` |
| Build podman échoue | Vérifier les permissions et le contexte de build |

## 📖 Conventions de Codage

- **Style** : Utiliser `rustfmt` pour le formatage
- **Longueur des lignes** : Limiter à 100 caractères
- **Commentaires** : En français pour la documentation
- **Noms** : `snake_case` pour variables/fonctions, `PascalCase` pour types/traits
- **Gestion d'erreurs** : Utiliser `anyhow/thiserror`, pas de `panic!` en production

## 🔧 Workflow de Développement Typique

1. Modifier le code dans `server/src/`
2. Vérifier avec : `cargo check`
3. Builder avec : `cargo build --release`
4. Tester avec : `cargo run --release --bin linky`
5. Builder podman : `podman build -t linky-c2 .`
6. Nettoyer avec : `cargo clean`

## 📚 Documentation Supplémentaire

- **`TODO.txt`** - Roadmap et tâches futures
- **`CLAUDE.md`** - Conventions et philosophie du projet
- **`README.md`** - Documentation principale

## 🎯 Objectifs du Projet

- **KISS** : Keep It Simple, Stupid
- **MVP** : Minimum Viable Product
- **Fiabilité** : Code robuste et testé
- **Sécurité** : Communication chiffrée et sécurisée
- **Maintenabilité** : Code propre et bien documenté

---

*Dernière mise à jour : 25 mars 2026*