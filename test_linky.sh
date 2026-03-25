#!/bin/bash

# Script de test automatisé pour Linky C2

set -e

echo "🔧 Démarrage des tests Linky C2..."

# 1. Vérification de base
echo "📋 Vérification du code..."
cd /home/fedora/Documents/linky
cargo check
cargo clippy

# 2. Build
echo "🔨 Construction..."
cargo build --release

# 3. Tests unitaires
echo "🧪 Exécution des tests unitaires..."
cargo test

# 4. Build podman
echo "🐳 Construction de l'image podman..."
podman build -t linky-c2 .

# 5. Vérification des fichiers clés
echo "📁 Vérification des fichiers clés..."
test -f server/src/cli.rs && echo "✓ cli.rs présent"
test -f server/src/routes.rs && echo "✓ routes.rs présent"
test -f server/src/links.rs && echo "✓ links.rs présent"

echo "✅ Tous les tests de base réussis!"

# Pour les tests d'intégration complets, exécuter:
echo "🚀 Pour tester avec un implant réel:"
echo "1. Démarrer le serveur: podman run -it --rm -p 8443:8443 -v ./implants:/implants:Z linky-c2"
echo "2. Générer un implant et exécuter des commandes"
echo "3. Vérifier que les résultats s'affichent correctement"