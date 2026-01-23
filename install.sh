#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

BINARY_NAME="proxyvpn"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
TARGET_DIR="${CARGO_TARGET_DIR:-target}"

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check if running as root for installation
check_root() {
    if [[ $EUID -ne 0 ]]; then
        error "This script must be run as root (for installation and setcap)"
    fi
}

# Check dependencies
check_deps() {
    info "Checking dependencies..."

    if ! command -v cargo &> /dev/null; then
        error "cargo not found. Please install Rust: https://rustup.rs"
    fi

    if ! command -v setcap &> /dev/null; then
        error "setcap not found. Please install libcap (libcap2-bin on Debian, libcap on Arch)"
    fi

    # Check for nft or iptables
    if ! command -v nft &> /dev/null && ! command -v iptables &> /dev/null; then
        warn "Neither nft nor iptables found. At least one is required at runtime."
    fi
}

# Build the project
build() {
    info "Building ${BINARY_NAME} in release mode..."
    info "Target directory: ${TARGET_DIR}"
    cargo build --release -p proxyvpn

    if [[ ! -f "${TARGET_DIR}/release/${BINARY_NAME}" ]]; then
        error "Build failed: binary not found at ${TARGET_DIR}/release/${BINARY_NAME}"
    fi

    info "Build successful!"
}

# Install the binary
install_binary() {
    info "Installing to ${INSTALL_DIR}..."

    # Create install directory if it doesn't exist
    mkdir -p "${INSTALL_DIR}"

    # Copy binary
    cp "${TARGET_DIR}/release/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod 755 "${INSTALL_DIR}/${BINARY_NAME}"

    info "Binary installed to ${INSTALL_DIR}/${BINARY_NAME}"
}

# Set capabilities
set_capabilities() {
    info "Setting CAP_NET_ADMIN capability..."

    setcap 'cap_net_admin=eip' "${INSTALL_DIR}/${BINARY_NAME}"

    # Verify
    if getcap "${INSTALL_DIR}/${BINARY_NAME}" | grep -q cap_net_admin; then
        info "Capability set successfully!"
    else
        error "Failed to set capability"
    fi
}

# Uninstall
uninstall() {
    info "Uninstalling ${BINARY_NAME}..."

    if [[ -f "${INSTALL_DIR}/${BINARY_NAME}" ]]; then
        rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        info "Removed ${INSTALL_DIR}/${BINARY_NAME}"
    else
        warn "Binary not found at ${INSTALL_DIR}/${BINARY_NAME}"
    fi
}

# Print usage
usage() {
    cat << EOF
Usage: $0 [COMMAND]

Commands:
    install     Build and install ${BINARY_NAME} (default)
    uninstall   Remove ${BINARY_NAME}
    build       Build only (no installation)
    help        Show this help

Environment variables:
    INSTALL_DIR        Installation directory (default: /usr/local/bin)
    CARGO_TARGET_DIR   Cargo target directory (default: target)

Examples:
    sudo $0                           # Build and install to /usr/local/bin
    sudo INSTALL_DIR=/opt/bin $0      # Install to custom directory
    sudo $0 uninstall                 # Remove installation
EOF
}

# Main
main() {
    case "${1:-install}" in
        install)
            check_root
            check_deps
            build
            install_binary
            set_capabilities
            echo
            info "Installation complete!"
            info "Run '${BINARY_NAME} --help' to get started"
            ;;
        uninstall)
            check_root
            uninstall
            ;;
        build)
            check_deps
            build
            ;;
        help|--help|-h)
            usage
            ;;
        *)
            error "Unknown command: $1"
            ;;
    esac
}

main "$@"
