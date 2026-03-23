#!/bin/bash
set -e

PROJECT_DIR="/home/fedora/Documents/linky"
OUTPUT_DIR="$PROJECT_DIR/test_implants"

mkdir -p "$OUTPUT_DIR"
cd "$PROJECT_DIR"

test_generate() {
    local target=$1
    local output_name=$2
    local cargo_target=$3

    cargo build --release --target "$cargo_target" -p "link-$target" > /dev/null 2>&1
    
    if [ -f "target/$cargo_target/release/$output_name" ]; then
        cp "target/$cargo_target/release/$output_name" "$OUTPUT_DIR/"
    else
        exit 1
    fi
}

test_generate "windows" "link-windows.exe" "x86_64-pc-windows-gnu"
test_generate "linux" "link-linux" "x86_64-unknown-linux-musl"

if [ -f "$OUTPUT_DIR/link-linux" ]; then
    chmod +x "$OUTPUT_DIR/link-linux"
    timeout 5 "$OUTPUT_DIR/link-linux" --test > /dev/null 2>&1 || true
fi
