#!/bin/bash
# Quick script to set capabilities after building
# Usage: ./scripts/set-caps.sh [debug|release]

BUILD_TYPE="${1:-release}"
CARGO_TARGET="${CARGO_TARGET_DIR:-$HOME/.cargo/target-cache}"

echo "Setting CAP_NET_ADMIN on $BUILD_TYPE binaries..."

for bin in proxyvpn proxyvpn-gui; do
    path="$CARGO_TARGET/$BUILD_TYPE/$bin"
    if [ -f "$path" ]; then
        sudo setcap 'cap_net_admin=eip' "$path" && echo "  ✓ $bin"
    fi
done

echo "Done! You can now run proxyvpn-gui without sudo."
