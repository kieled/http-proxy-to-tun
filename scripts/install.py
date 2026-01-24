#!/usr/bin/env python3
"""
Cross-distribution installer for http-tun.

Supports:
    - Arch Linux (pacman)
    - Debian/Ubuntu (apt)
    - Fedora/RHEL (dnf)
    - Generic Linux (manual binary install)

Usage:
    sudo ./scripts/install.py              # Install from built binaries
    sudo ./scripts/install.py --uninstall  # Remove installation
    sudo ./scripts/install.py --deps-only  # Install dependencies only
"""

import argparse
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from enum import Enum, auto
from pathlib import Path
from typing import Optional

# Installation paths
PREFIX = Path("/usr/local")
BIN_DIR = PREFIX / "bin"
SHARE_DIR = PREFIX / "share"
APPS_DIR = SHARE_DIR / "applications"
ICONS_DIR = SHARE_DIR / "icons" / "hicolor"

# Project paths
ROOT = Path(__file__).parent.parent.resolve()

# Respect CARGO_TARGET_DIR if set
_cargo_target = os.environ.get("CARGO_TARGET_DIR")
if _cargo_target:
    TARGET_DIR = Path(_cargo_target)
else:
    TARGET_DIR = ROOT / "target"

TARGET_RELEASE = TARGET_DIR / "release"
TAURI_BUNDLE = TARGET_RELEASE / "bundle"


class Distro(Enum):
    """Supported Linux distributions."""
    ARCH = auto()
    DEBIAN = auto()
    FEDORA = auto()
    GENERIC = auto()


@dataclass
class DistroInfo:
    """Distribution information."""
    distro: Distro
    name: str
    version: str = ""
    pkg_manager: str = ""

    @staticmethod
    def detect() -> "DistroInfo":
        """Detect the current Linux distribution."""
        # Check /etc/os-release
        os_release = {}
        try:
            with open("/etc/os-release") as f:
                for line in f:
                    if "=" in line:
                        key, value = line.strip().split("=", 1)
                        os_release[key] = value.strip('"')
        except FileNotFoundError:
            pass

        distro_id = os_release.get("ID", "").lower()
        distro_like = os_release.get("ID_LIKE", "").lower()
        name = os_release.get("NAME", "Linux")
        version = os_release.get("VERSION_ID", "")

        # Arch Linux
        if distro_id == "arch" or "arch" in distro_like:
            return DistroInfo(Distro.ARCH, name, version, "pacman")

        # Debian/Ubuntu
        if distro_id in ("debian", "ubuntu") or "debian" in distro_like:
            return DistroInfo(Distro.DEBIAN, name, version, "apt")

        # Fedora/RHEL
        if distro_id in ("fedora", "rhel", "centos", "rocky", "alma") or "fedora" in distro_like:
            return DistroInfo(Distro.FEDORA, name, version, "dnf")

        return DistroInfo(Distro.GENERIC, name, version, "")


def run(cmd: list[str], check: bool = True, capture: bool = False) -> subprocess.CompletedProcess:
    """Run a command."""
    if capture:
        return subprocess.run(cmd, check=check, capture_output=True, text=True)
    return subprocess.run(cmd, check=check)


def ensure_root() -> None:
    """Ensure running as root."""
    if os.geteuid() != 0:
        print("🔐 This script requires root privileges.")
        print("   Run with: sudo ./scripts/install.py")
        sys.exit(1)


def has_command(cmd: str) -> bool:
    """Check if command exists."""
    return shutil.which(cmd) is not None


# =============================================================================
# Dependencies
# =============================================================================

ARCH_DEPS = [
    "base-devel",
    "webkit2gtk-4.1",
    "libayatana-appindicator",
    "librsvg",
    "fuse2",  # Required for AppImage bundling
    "file",   # Required for linuxdeploy
]

DEBIAN_DEPS = [
    "build-essential",
    "libwebkit2gtk-4.1-dev",
    "libayatana-appindicator3-dev",
    "librsvg2-dev",
    "libssl-dev",
    "pkg-config",
]

FEDORA_DEPS = [
    "webkit2gtk4.1-devel",
    "libayatana-appindicator-gtk3-devel",
    "librsvg2-devel",
    "openssl-devel",
    "pkg-config",
]

# Cargo tools to install
CARGO_TOOLS = [
    "tauri-cli",
]


def install_bun() -> None:
    """Install bun if not present."""
    if has_command("bun"):
        result = run(["bun", "--version"], check=False, capture=True)
        if result.returncode == 0:
            print(f"→ bun already installed ({result.stdout.strip()})")
            return

    print("→ Installing bun...")
    # Use the official bun installer
    run(["bash", "-c", "curl -fsSL https://bun.sh/install | bash"], check=True)
    print("  ⚠️  You may need to restart your shell or run: source ~/.bashrc")


def install_cargo_tools() -> None:
    """Install required Cargo tools."""
    if not has_command("cargo"):
        print("⚠️  Cargo not found, skipping cargo tools")
        return

    for tool in CARGO_TOOLS:
        # Check if already installed
        bin_name = tool.replace("-", " ").split()[0]  # tauri-cli -> tauri
        if tool == "tauri-cli":
            bin_name = "tauri"

        result = run(["cargo", bin_name, "--version"], check=False, capture=True)
        if result.returncode == 0:
            print(f"→ {tool} already installed")
            continue

        print(f"→ Installing {tool} via cargo...")
        run(["cargo", "install", tool])


def install_deps_arch() -> None:
    """Install dependencies on Arch Linux."""
    print("→ Installing Arch Linux dependencies...")
    run(["pacman", "-S", "--needed", "--noconfirm"] + ARCH_DEPS)


def install_deps_debian() -> None:
    """Install dependencies on Debian/Ubuntu."""
    print("→ Updating package lists...")
    run(["apt", "update"])
    print("→ Installing Debian/Ubuntu dependencies...")
    run(["apt", "install", "-y"] + DEBIAN_DEPS)


def install_deps_fedora() -> None:
    """Install dependencies on Fedora/RHEL."""
    print("→ Installing Fedora dependencies...")
    run(["dnf", "install", "-y"] + FEDORA_DEPS)


def install_deps(distro: DistroInfo) -> None:
    """Install build dependencies for the detected distro."""
    print(f"\n📦 Installing dependencies for {distro.name}...\n")

    if distro.distro == Distro.ARCH:
        install_deps_arch()
    elif distro.distro == Distro.DEBIAN:
        install_deps_debian()
    elif distro.distro == Distro.FEDORA:
        install_deps_fedora()
    else:
        print("⚠️  Unknown distribution. Please install dependencies manually:")
        print("   - WebKit2GTK 4.1")
        print("   - libayatana-appindicator")
        print("   - librsvg")
        print("   - OpenSSL development headers")

    # Install bun (JavaScript runtime/package manager)
    print("\n→ Checking bun...")
    install_bun()

    # Install cargo tools (tauri-cli, etc.)
    print("\n→ Checking cargo tools...")
    install_cargo_tools()

    print("\n✅ Dependencies installed!")


# =============================================================================
# Installation
# =============================================================================

def find_binary(name: str) -> Optional[Path]:
    """Find built binary."""
    # Check release build
    path = TARGET_RELEASE / name
    if path.exists():
        return path

    # Check Tauri bundle
    for bundle_type in ["deb", "rpm", "appimage"]:
        bundle_dir = TAURI_BUNDLE / bundle_type
        if bundle_dir.exists():
            for item in bundle_dir.iterdir():
                if item.name.startswith(name):
                    return item

    return None


def install_binary(src: Path, name: str, set_caps: bool = False) -> None:
    """Install a binary to BIN_DIR."""
    dest = BIN_DIR / name
    print(f"→ Installing {name} to {dest}")

    BIN_DIR.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dest)
    dest.chmod(0o755)

    if set_caps:
        set_capabilities(dest)


def set_capabilities(binary: Path) -> None:
    """Set CAP_NET_ADMIN capability on binary."""
    if not has_command("setcap"):
        print("⚠️  setcap not found, skipping capability setting")
        print("   Install libcap (or run as root)")
        return

    print(f"→ Setting CAP_NET_ADMIN on {binary}")
    try:
        run(["setcap", "cap_net_admin+ep", str(binary)])
    except subprocess.CalledProcessError:
        print("⚠️  Failed to set capabilities")
        print("   You may need to run with sudo or setuid")


def install_desktop_file() -> None:
    """Install desktop file for application menu."""
    APPS_DIR.mkdir(parents=True, exist_ok=True)

    desktop_content = """\
[Desktop Entry]
Name=HTTP Tunnel
Comment=System-wide HTTP proxy VPN
Exec=http-tun
Icon=http-tun
Terminal=false
Type=Application
Categories=Network;VPN;
Keywords=proxy;vpn;tunnel;http;
StartupNotify=true
"""

    desktop_file = APPS_DIR / "http-tun.desktop"
    print(f"→ Installing desktop file to {desktop_file}")
    desktop_file.write_text(desktop_content)
    desktop_file.chmod(0o644)


def install_icons() -> None:
    """Install application icons."""
    # Icon source from Tauri
    icons_src = ROOT / "crates" / "tauri-app" / "icons"

    icon_sizes = [
        ("32x32.png", "32x32"),
        ("128x128.png", "128x128"),
        ("128x128@2x.png", "256x256"),
    ]

    for src_name, size in icon_sizes:
        src = icons_src / src_name
        if not src.exists():
            continue

        dest_dir = ICONS_DIR / size / "apps"
        dest_dir.mkdir(parents=True, exist_ok=True)
        dest = dest_dir / "http-tun.png"

        print(f"→ Installing {size} icon")
        shutil.copy2(src, dest)

    # Update icon cache
    if has_command("gtk-update-icon-cache"):
        run(["gtk-update-icon-cache", "-f", str(ICONS_DIR)], check=False)


def install_from_binaries() -> None:
    """Install from built binaries."""
    print("\n📦 Installing http-tun...\n")

    # Find and install main binaries
    # Format: (binary_name, needs_caps, install_as)
    binaries = [
        ("http-tun-desktop", True, "http-tun"),  # Desktop app -> http-tun
        ("proxyvpn", True, None),                 # CLI tool (needs CAP_NET_ADMIN)
        ("proxyvpn-cli", False, None),            # Helper CLI
    ]

    installed = []
    for name, needs_caps, install_as in binaries:
        binary = find_binary(name)
        if binary:
            target_name = install_as or name
            install_binary(binary, target_name, set_caps=needs_caps)
            installed.append(target_name)
        else:
            print(f"⚠️  Binary not found: {name}")
            print(f"   Build first with: ./tasks.py build")

    if not installed:
        print("\n❌ No binaries found to install!")
        print("   Build first with: ./tasks.py build")
        sys.exit(1)

    # Install desktop integration
    install_desktop_file()
    install_icons()

    # Update desktop database
    if has_command("update-desktop-database"):
        run(["update-desktop-database", str(APPS_DIR)], check=False)

    print(f"\n✅ Installed: {', '.join(installed)}")
    print(f"   Location: {BIN_DIR}")


def install_from_package(distro: DistroInfo) -> None:
    """Install from distribution package if available."""

    # Check for .deb in bundle (Debian/Ubuntu)
    if distro.distro == Distro.DEBIAN:
        deb_dir = TAURI_BUNDLE / "deb"
        if deb_dir.exists():
            debs = list(deb_dir.glob("*.deb"))
            if debs:
                print(f"→ Installing {debs[0].name}...")
                run(["dpkg", "-i", str(debs[0])])
                return

    # Check for .rpm in bundle (Fedora/RHEL)
    elif distro.distro == Distro.FEDORA:
        rpm_dir = TAURI_BUNDLE / "rpm"
        if rpm_dir.exists():
            rpms = list(rpm_dir.glob("*.rpm"))
            if rpms:
                print(f"→ Installing {rpms[0].name}...")
                run(["rpm", "-i", str(rpms[0])])
                return

    # Default: install from built binaries
    install_from_binaries()


# =============================================================================
# Uninstallation
# =============================================================================

def uninstall() -> None:
    """Remove http-tun installation."""
    print("\n🗑️  Uninstalling http-tun...\n")

    # Remove binaries
    binaries = ["http-tun", "proxyvpn", "proxyvpn-cli"]
    for name in binaries:
        path = BIN_DIR / name
        if path.exists():
            print(f"→ Removing {path}")
            path.unlink()

    # Remove desktop file
    desktop_file = APPS_DIR / "http-tun.desktop"
    if desktop_file.exists():
        print(f"→ Removing {desktop_file}")
        desktop_file.unlink()

    # Remove icons
    for size in ["32x32", "128x128", "256x256"]:
        icon = ICONS_DIR / size / "apps" / "http-tun.png"
        if icon.exists():
            print(f"→ Removing {icon}")
            icon.unlink()

    # Update caches
    if has_command("update-desktop-database"):
        run(["update-desktop-database", str(APPS_DIR)], check=False)
    if has_command("gtk-update-icon-cache"):
        run(["gtk-update-icon-cache", "-f", str(ICONS_DIR)], check=False)

    print("\n✅ Uninstalled!")


# =============================================================================
# Main
# =============================================================================

def install_from_aur(aur_helper: str, package: str = "http-tun") -> bool:
    """Try to install from AUR. Returns True if successful."""
    print(f"→ Installing from AUR using {aur_helper}...")
    result = run([aur_helper, "-S", "--noconfirm", package], check=False)
    return result.returncode == 0


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Install http-tun on Linux",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--deps-only", action="store_true",
                        help="Only install dependencies")
    parser.add_argument("--uninstall", action="store_true",
                        help="Uninstall http-tun")
    parser.add_argument("--no-deps", action="store_true",
                        help="Skip dependency installation")
    parser.add_argument("--binary", action="store_true",
                        help="Install from built binaries")
    parser.add_argument("--aur", action="store_true",
                        help="Install from AUR (Arch Linux only)")
    parser.add_argument("--target-dir", type=Path,
                        help="Cargo target directory (default: ./target or CARGO_TARGET_DIR)")

    args = parser.parse_args()

    # Override target paths if --target-dir is specified
    global TARGET_DIR, TARGET_RELEASE, TAURI_BUNDLE
    if args.target_dir:
        TARGET_DIR = args.target_dir
        TARGET_RELEASE = TARGET_DIR / "release"
        TAURI_BUNDLE = TARGET_RELEASE / "bundle"

    ensure_root()

    distro = DistroInfo.detect()
    print(f"🐧 Detected: {distro.name}")

    if args.uninstall:
        uninstall()
        return

    if not args.no_deps:
        install_deps(distro)

    if args.deps_only:
        return

    # AUR install (explicit flag only)
    if args.aur:
        if distro.distro != Distro.ARCH:
            print("❌ --aur is only supported on Arch Linux")
            sys.exit(1)

        aur_helper = None
        for helper in ["paru", "yay", "pikaur"]:
            if has_command(helper):
                aur_helper = helper
                break

        if not aur_helper:
            print("❌ No AUR helper found (paru, yay, pikaur)")
            sys.exit(1)

        if not install_from_aur(aur_helper):
            print("❌ AUR installation failed")
            sys.exit(1)
        return

    # Default: install from local build
    install_from_package(distro)


if __name__ == "__main__":
    main()
