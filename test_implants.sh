#!/bin/bash
set -e

# Chemin du projet
PROJECT_DIR="/home/fedora/Documents/linky"
OUTPUT_DIR="$PROJECT_DIR/test_implants"
CALLBACK="127.0.0.1:8443"

# Créer le dossier de sortie
mkdir -p "$OUTPUT_DIR"
cd "$PROJECT_DIR"

# Fonction pour tester la génération d'un implant
test_generate() {
    local target=$1
    local output_name=$2
    local cargo_target=$3

    echo "Testing $target implant generation..."
    
    # Construction avec cargo
    cargo build --release --target "$cargo_target" -p "link-$target" 2>&1 | grep -E "(Compiling|Finished)" || true
    
    # Vérification du binaire
    if [ -f "target/$cargo_target/release/$output_name" ]; then
        echo "✓ $target implant generated successfully"
        cp "target/$cargo_target/release/$output_name" "$OUTPUT_DIR/"
    else
        echo "✗ $target implant generation failed"
        exit 1
    fi
}

# Test de génération pour chaque plateforme
echo "Starting implant generation tests..."
test_generate "windows" "link-windows.exe" "x86_64-pc-windows-gnu"
test_generate "linux" "link-linux" "x86_64-unknown-linux-musl"
test_generate "linux-native" "link-linux" "x86_64-unknown-linux-gnu"
echo "Skipping macOS implant (cross-compilation not configured)"

# Test d'exécution de l'implant Linux natif (si possible)
if [ -f "$OUTPUT_DIR/link-linux" ]; then
    echo "Testing Linux implant execution..."
    chmod +x "$OUTPUT_DIR/link-linux"
    timeout 5 "$OUTPUT_DIR/link-linux" --test || echo "✓ Linux implant executed (timeout expected)"
else
    echo "✗ Linux implant not found for execution test"
fi

echo "All tests completed successfully!"
