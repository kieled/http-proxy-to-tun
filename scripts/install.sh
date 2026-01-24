#!/usr/bin/env bash
#
# Quick install script for http-tun
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/kieled/http-proxy-to-tun/main/scripts/install.sh | bash
#
# Or to install specific version:
#   curl -fsSL ... | bash -s -- --version v0.1.0
#

set -euo pipefail

REPO="user/http-tun"  # TODO: Update with actual repo
VERSION="${VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { echo -e "${BLUE}→${NC} $*"; }
success() { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC} $*"; }
error() { echo -e "${RED}✗${NC} $*" >&2; }

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version|-v)
            VERSION="$2"
            shift 2
            ;;
        --dir|-d)
            INSTALL_DIR="$2"
            shift 2
            ;;
        *)
            error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Detect OS and architecture
detect_platform() {
    local os arch

    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux) os="linux" ;;
        darwin) os="macos" ;;
        *)
            error "Unsupported OS: $os"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)
            error "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    echo "${os}-${arch}"
}

# Detect Linux distribution
detect_distro() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        echo "${ID:-linux}"
    else
        echo "linux"
    fi
}

# Check for required commands
check_deps() {
    local missing=()

    for cmd in curl tar; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing required commands: ${missing[*]}"
        exit 1
    fi
}

# Install bun if not present
install_bun() {
    if command -v bun &>/dev/null; then
        info "bun already installed ($(bun --version))"
        return
    fi

    info "Installing bun..."
    curl -fsSL https://bun.sh/install | bash

    # Source bun for current session
    export BUN_INSTALL="$HOME/.bun"
    export PATH="$BUN_INSTALL/bin:$PATH"

    if command -v bun &>/dev/null; then
        success "bun installed ($(bun --version))"
    else
        warn "bun installed but not in PATH. Restart your shell or run: source ~/.bashrc"
    fi
}

# Download and extract release
download_release() {
    local platform="$1"
    local version="$2"
    local url archive tmp_dir

    if [[ "$version" == "latest" ]]; then
        url="https://github.com/${REPO}/releases/latest/download/http-tun-${platform}.tar.gz"
    else
        url="https://github.com/${REPO}/releases/download/${version}/http-tun-${platform}.tar.gz"
    fi

    info "Downloading from $url..."

    tmp_dir="$(mktemp -d)"
    archive="${tmp_dir}/http-tun.tar.gz"

    if ! curl -fsSL "$url" -o "$archive"; then
        error "Failed to download release"
        rm -rf "$tmp_dir"
        exit 1
    fi

    info "Extracting..."
    tar -xzf "$archive" -C "$tmp_dir"

    echo "$tmp_dir"
}

# Install binaries
install_binaries() {
    local tmp_dir="$1"
    local need_sudo=""

    # Check if we need sudo
    if [[ ! -w "$INSTALL_DIR" ]]; then
        if command -v sudo &>/dev/null; then
            need_sudo="sudo"
            info "Installation requires sudo..."
        else
            error "Cannot write to $INSTALL_DIR and sudo not available"
            exit 1
        fi
    fi

    # Create install directory
    $need_sudo mkdir -p "$INSTALL_DIR"

    # Install binaries
    for bin in http-tun proxyvpn proxyvpn-cli; do
        if [[ -f "${tmp_dir}/${bin}" ]]; then
            info "Installing ${bin}..."
            $need_sudo install -m 755 "${tmp_dir}/${bin}" "${INSTALL_DIR}/${bin}"

            # Set capabilities for binaries that need CAP_NET_ADMIN
            if [[ "$bin" == "http-tun" || "$bin" == "proxyvpn" ]]; then
                if command -v setcap &>/dev/null; then
                    $need_sudo setcap cap_net_admin+ep "${INSTALL_DIR}/${bin}" 2>/dev/null || \
                        warn "Could not set CAP_NET_ADMIN on ${bin}"
                fi
            fi
        fi
    done

    # Cleanup
    rm -rf "$tmp_dir"
}

# Install desktop file (Linux only)
install_desktop() {
    local need_sudo=""
    local apps_dir="/usr/local/share/applications"

    [[ "$(uname -s)" != "Linux" ]] && return

    if [[ ! -w "$apps_dir" ]]; then
        if command -v sudo &>/dev/null; then
            need_sudo="sudo"
        else
            return
        fi
    fi

    $need_sudo mkdir -p "$apps_dir"

    cat <<EOF | $need_sudo tee "${apps_dir}/http-tun.desktop" >/dev/null
[Desktop Entry]
Name=HTTP Tunnel
Comment=System-wide HTTP proxy VPN
Exec=http-tun
Icon=http-tun
Terminal=false
Type=Application
Categories=Network;VPN;
EOF

    success "Desktop file installed"
}

main() {
    echo -e "${GREEN}"
    echo "  _   _ _____ _____ ____    _____               "
    echo " | | | |_   _|_   _|  _ \  |_   _|   _ _ __     "
    echo " | |_| | | |   | | | |_) |   | || | | | '_ \    "
    echo " |  _  | | |   | | |  __/    | || |_| | | | |   "
    echo " |_| |_| |_|   |_| |_|       |_| \__,_|_| |_|   "
    echo -e "${NC}"
    echo "  System-wide HTTP Proxy VPN"
    echo ""

    check_deps

    local platform distro
    platform="$(detect_platform)"
    distro="$(detect_distro)"

    info "Platform: $platform"
    info "Distribution: $distro"
    info "Version: $VERSION"
    info "Install directory: $INSTALL_DIR"
    echo ""

    # Download and install
    local tmp_dir
    tmp_dir="$(download_release "$platform" "$VERSION")"
    install_binaries "$tmp_dir"
    install_desktop

    echo ""
    success "Installation complete!"
    echo ""
    echo "  Run 'http-tun' to start the GUI"
    echo "  Run 'proxyvpn --help' for CLI usage"
    echo ""

    # Check if in PATH
    if ! command -v http-tun &>/dev/null; then
        warn "Note: $INSTALL_DIR may not be in your PATH"
        echo "  Add to your shell config: export PATH=\"\$PATH:$INSTALL_DIR\""
    fi
}

main "$@"
