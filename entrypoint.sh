#!/bin/bash

# Check if implants exist in /usr/local/implants (only present in DEV_MODE builds)
if [ -f "/usr/local/implants/link-windows.exe" ] || [ -f "/usr/local/implants/link-linux" ]; then
    echo "[*] Implants found in /usr/local/implants (DEV_MODE build)"
    
    # Copy implants from /usr/local/implants to /implants if they don't exist
    if [ ! -f "/implants/link-windows.exe" ] && [ -f "/usr/local/implants/link-windows.exe" ]; then
        echo "[*] Copying Windows implant..."
        cp /usr/local/implants/link-windows.exe /implants/ 2>/dev/null || {
            echo "[!] Permission denied for /implants, copying to /tmp instead"
            cp /usr/local/implants/link-windows.exe /tmp/
            echo "[!] Windows implant available in /tmp/ inside container"
            ;
        }
    fi
    
    if [ ! -f "/implants/link-linux" ] && [ -f "/usr/local/implants/link-linux" ]; then
        echo "[*] Copying Linux implant..."
        cp /usr/local/implants/link-linux /implants/ 2>/dev/null || {
            echo "[!] Permission denied for /implants, copying to /tmp instead"
            cp /usr/local/implants/link-linux /tmp/
            echo "[!] Linux implant available in /tmp/ inside container"
            ;
        }
    fi
else
    echo "[*] No implants found in /usr/local/implants (production mode)"
    echo "[*] To generate implants, rebuild with: docker build --build-arg DEV_MODE=true ."
fi

# Start the server
exec "$@"