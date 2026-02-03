#!/bin/bash
# Setup script for proxyvpn-gui capabilities
# This script configures the system to allow the current user to set
# CAP_NET_ADMIN capability on proxyvpn binaries without password.
#
# Usage: sudo ./scripts/setup-capabilities.sh
#
# After running this script, you can use:
#   ./scripts/set-caps.sh
# to set capabilities without entering your password.

set -e

if [ "$EUID" -ne 0 ]; then
    echo "This script must be run as root (use sudo)"
    exit 1
fi

USER=${SUDO_USER:-$USER}
CARGO_TARGET="${CARGO_TARGET_DIR:-$HOME/.cargo/target-cache}"

echo "Setting up capability configuration for user: $USER"

# Create a script that sets the capability
cat > /usr/local/bin/proxyvpn-setcap << 'SETCAP_SCRIPT'
#!/bin/bash
# Set CAP_NET_ADMIN capability on proxyvpn binaries
CARGO_TARGET="${CARGO_TARGET_DIR:-$HOME/.cargo/target-cache}"

for build_type in debug release; do
    for bin in proxyvpn proxyvpn-gui; do
        path="$CARGO_TARGET/$build_type/$bin"
        if [ -f "$path" ]; then
            setcap 'cap_net_admin=eip' "$path"
            echo "Set capability on: $path"
        fi
    done
done
SETCAP_SCRIPT

chmod +x /usr/local/bin/proxyvpn-setcap

# Create sudoers file allowing the user to run setcap without password
cat > /etc/sudoers.d/proxyvpn-caps << EOF
# Allow $USER to set capabilities on proxyvpn binaries without password
$USER ALL=(root) NOPASSWD: /usr/local/bin/proxyvpn-setcap
$USER ALL=(root) NOPASSWD: /usr/sbin/setcap cap_net_admin=eip *proxyvpn*
EOF

chmod 440 /etc/sudoers.d/proxyvpn-caps

echo ""
echo "Setup complete! You can now use:"
echo "  sudo proxyvpn-setcap"
echo ""
echo "Or add this to your build process after cargo build:"
echo "  cargo build --release && sudo proxyvpn-setcap"
echo ""
echo "For automatic capability setting after each build, add this to your shell profile:"
echo '  alias cargo-proxyvpn="cargo build -p proxyvpn-iced --release && sudo proxyvpn-setcap"'
