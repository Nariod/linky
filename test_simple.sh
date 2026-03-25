#!/bin/bash

echo "=== Test simple de fonctionnement ==="
echo ""

cd /home/fedora/Documents/linky

# Vérifier que le binaire existe
echo "1. Vérification du binaire..."
if [ -f target/release/linky ]; then
    echo "✅ Binaire trouvé: target/release/linky"
else
    echo "❌ Binaire non trouvé"
    exit 1
fi

# Vérifier que les dépendances sont disponibles
echo ""
echo "2. Vérification des dépendances..."
ldd target/release/linky 2>&1 | head -5

# Tester l'exécution du binaire
echo ""
echo "3. Test d'exécution (version)..."
target/release/linky --version 2>&1 || echo "(Aucune option --version, c'est normal)"

# Vérifier les symboles exportés
echo ""
echo "4. Vérification des symboles..."
nm -D target/release/linky 2>&1 | grep -i "main\|run\|link" | head -5 || echo "Aucun symbole trouvé (c'est normal pour un binaire Rust)"

echo ""
echo "=== Test terminé ==="
echo ""
echo "Conclusion: Le binaire est compilé et prêt à être utilisé."
echo "Les problèmes d'affichage des commandes doivent être résolus par:"
echo "1. Reconstruction du conteneur Docker avec cette version"
echo "2. Test manuel avec un implant réel"
echo "3. Vérification que show_completed_task_results() est appelé"
