#!/bin/bash

# Copy implants from builder stage if they exist
set +e  # Continue even if files don't exist

if [ -f "/app/target/x86_64-pc-windows-gnu/release/link-windows.exe" ]; then
    cp "/app/target/x86_64-pc-windows-gnu/release/link-windows.exe" /usr/local/implants/
fi

if [ -f "/app/target/x86_64-unknown-linux-musl/release/link-linux" ]; then
    cp "/app/target/x86_64-unknown-linux-musl/release/link-linux" /usr/local/implants/
fi

set -e  # Reset error handling