#!/usr/bin/env python3
"""
Build task runner for http-tun project.

Usage:
    python tasks.py <task> [options]
    ./tasks.py <task> [options]

Tasks:
    dev         Start development servers (UI + Tauri)
    build       Build production release
    test        Run all tests
    lint        Run linters
    format      Format code
    clean       Clean build artifacts
    install     Install the application
    release     Build and package for release
"""

import argparse
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

# Project paths
ROOT = Path(__file__).parent.resolve()
UI_DIR = ROOT / "ui"
TAURI_DIR = ROOT / "crates" / "tauri-app"
SCRIPTS_DIR = ROOT / "scripts"
TARGET_DIR = ROOT / "target"


@dataclass
class Config:
    """Build configuration."""

    verbose: bool = False
    release: bool = True
    parallel: bool = True


def run(
    cmd: list[str],
    cwd: Optional[Path] = None,
    env: Optional[dict] = None,
    check: bool = True,
    capture: bool = False,
) -> subprocess.CompletedProcess:
    """Run a command with proper error handling."""
    merged_env = {**os.environ, **(env or {})}
    cwd = cwd or ROOT

    if capture:
        return subprocess.run(
            cmd,
            cwd=cwd,
            env=merged_env,
            check=check,
            capture_output=True,
            text=True,
        )

    result = subprocess.run(cmd, cwd=cwd, env=merged_env, check=False)
    if check and result.returncode != 0:
        print(f"\n❌ Command failed: {' '.join(cmd)}")
        sys.exit(result.returncode)
    return result


def has_command(cmd: str) -> bool:
    """Check if a command is available."""
    return shutil.which(cmd) is not None


def ensure_command(cmd: str, install_hint: str = "") -> None:
    """Ensure a command is available, exit with helpful message if not."""
    if not has_command(cmd):
        msg = f"❌ Required command not found: {cmd}"
        if install_hint:
            msg += f"\n   Install: {install_hint}"
        print(msg)
        sys.exit(1)


# =============================================================================
# Tasks
# =============================================================================


def task_dev(cfg: Config) -> None:
    """Start development servers."""
    ensure_command("bun", "https://bun.sh")
    ensure_command("cargo")

    print("🚀 Starting development servers...")
    print("   UI:    http://localhost:5173")
    print("   Tauri: bunx tauri dev\n")

    # Start UI dev server in background
    ui_proc = subprocess.Popen(
        ["bun", "run", "dev"],
        cwd=UI_DIR,
        env={**os.environ, "FORCE_COLOR": "1"},
    )

    try:
        # Start Tauri dev from root (finds config in crates/tauri-app/)
        run(["bunx", "@tauri-apps/cli", "dev"], cwd=ROOT)
    finally:
        ui_proc.terminate()
        ui_proc.wait()


def task_build(cfg: Config) -> None:
    """Build production release."""
    ensure_command("bun", "https://bun.sh")
    ensure_command("cargo")

    print("📦 Building production release...")

    # Build UI
    print("\n→ Building UI...")
    run(["bun", "run", "build"], cwd=UI_DIR)

    # Build Tauri app from root (finds config in crates/tauri-app/)
    print("\n→ Building Tauri app...")
    args = ["bunx", "@tauri-apps/cli", "build"]
    if cfg.verbose:
        args.append("--verbose")
    # NO_STRIP: linuxdeploy's bundled strip is too old for modern glibc
    # APPIMAGE_EXTRACT_AND_RUN: works around FUSE issues on some systems
    run(args, cwd=ROOT, env={
        "NO_STRIP": "1",
        "APPIMAGE_EXTRACT_AND_RUN": "1",
    })

    print("\n✅ Build complete!")
    print(f"   Output: {TARGET_DIR / 'release' / 'bundle'}")


def task_build_cli(cfg: Config) -> None:
    """Build CLI tools only."""
    ensure_command("cargo")

    print("📦 Building CLI...")
    args = ["cargo", "build", "-p", "proxyvpn", "-p", "proxyvpn-cli"]
    if cfg.release:
        args.append("--release")
    if cfg.verbose:
        args.append("--verbose")
    run(args, cwd=ROOT)

    print("\n✅ CLI build complete!")


def task_test(cfg: Config) -> None:
    """Run all tests."""
    ensure_command("cargo")
    ensure_command("bun", "https://bun.sh")

    print("🧪 Running tests...\n")

    # Rust tests
    print("→ Rust unit tests...")
    run(["cargo", "test", "--workspace"], cwd=ROOT)

    # UI tests/lint
    print("\n→ UI lint...")
    run(["bun", "run", "lint"], cwd=UI_DIR)

    print("\n✅ All tests passed!")


def task_test_privileged(cfg: Config) -> None:
    """Run privileged tests (requires root)."""
    ensure_command("cargo")

    if os.geteuid() != 0:
        print("🔐 Elevating to root...")
        os.execvp("sudo", ["sudo", "-E", sys.executable, __file__, "test:privileged"])

    print("🧪 Running privileged tests...\n")

    env = {
        "PROXYVPN_PRIV_TESTS_ALLOW_FIREWALL": "1",
        "PROXYVPN_PRIV_TESTS_ALLOW_MARK": "1",
        "PROXYVPN_PRIV_TESTS_ALLOW_NETLINK": "1",
        "PROXYVPN_PRIV_TESTS_ALLOW_DNS": "1",
    }

    packages = [
        "proxyvpn-firewall",
        "proxyvpn-mark",
        "proxyvpn-netlink",
        "proxyvpn-selftest",
    ]

    for pkg in packages:
        print(f"→ Testing {pkg}...")
        run(
            [
                "cargo",
                "test",
                "-p",
                pkg,
                "--features",
                "privileged-tests",
                "--",
                "--ignored",
            ],
            cwd=ROOT,
            env=env,
        )

    print("\n✅ Privileged tests passed!")


def task_test_e2e(cfg: Config) -> None:
    """Run end-to-end tests."""
    proxy_url = os.environ.get("PROXYVPN_E2E_PROXY_URL")
    if not proxy_url:
        print("❌ PROXYVPN_E2E_PROXY_URL is required")
        print(
            "   Example: PROXYVPN_E2E_PROXY_URL=http://user:pass@proxy:8080 ./tasks.py test:e2e"
        )
        sys.exit(1)

    # Run the Python e2e script (handles root elevation internally)
    run([sys.executable, str(SCRIPTS_DIR / "test_e2e.py")], cwd=ROOT)


def task_lint(cfg: Config) -> None:
    """Run all linters."""
    ensure_command("cargo")
    ensure_command("bun", "https://bun.sh")

    print("🔍 Running linters...\n")

    # Rust
    print("→ Cargo clippy...")
    run(["cargo", "clippy", "--workspace", "--", "-D", "warnings"], cwd=ROOT)

    print("\n→ Cargo fmt check...")
    run(["cargo", "fmt", "--check"], cwd=ROOT)

    # UI
    print("\n→ Biome check...")
    run(["bun", "run", "lint"], cwd=UI_DIR)

    print("\n✅ All linters passed!")


def task_format(cfg: Config) -> None:
    """Format all code."""
    ensure_command("cargo")
    ensure_command("bun", "https://bun.sh")

    print("✨ Formatting code...\n")

    # Rust
    print("→ Cargo fmt...")
    run(["cargo", "fmt"], cwd=ROOT)

    # UI
    print("→ Biome format...")
    run(["bun", "run", "format"], cwd=UI_DIR)

    print("\n✅ Formatting complete!")


def task_clean(cfg: Config) -> None:
    """Clean build artifacts."""
    print("🧹 Cleaning build artifacts...\n")

    # Cargo clean
    if TARGET_DIR.exists():
        print(f"→ Removing {TARGET_DIR}...")
        run(["cargo", "clean"], cwd=ROOT)

    # UI dist
    ui_dist = UI_DIR / "dist"
    if ui_dist.exists():
        print(f"→ Removing {ui_dist}...")
        shutil.rmtree(ui_dist)

    # Node modules (optional, only if --full)
    print("\n✅ Clean complete!")


def task_deps_ui(cfg: Config) -> None:
    """Install UI (npm) dependencies."""
    ensure_command("bun", "https://bun.sh")

    print("📥 Installing UI dependencies...\n")
    run(["bun", "install"], cwd=UI_DIR)

    print("\n✅ UI dependencies installed!")


def task_release(cfg: Config) -> None:
    """Build release packages."""
    print("📦 Building release packages...\n")

    # Clean first
    task_clean(cfg)

    # Build
    task_build(cfg)

    # Find bundles
    bundle_dir = TARGET_DIR / "release" / "bundle"
    if bundle_dir.exists():
        print("\n📦 Release packages:")
        for item in bundle_dir.iterdir():
            if item.is_dir():
                for pkg in item.iterdir():
                    size = pkg.stat().st_size / (1024 * 1024)
                    print(f"   {pkg.name} ({size:.1f} MB)")

    print("\n✅ Release complete!")


def task_debug_collect(cfg: Config) -> None:
    """Collect debug information."""
    # Run the Python debug script (handles root elevation internally)
    run([sys.executable, str(SCRIPTS_DIR / "collect_debug.py")], cwd=ROOT)


def task_install(cfg: Config) -> None:
    """Install http-tun to system."""
    # Detect target dir (respect CARGO_TARGET_DIR)
    target_dir = os.environ.get("CARGO_TARGET_DIR", str(TARGET_DIR))
    # Run the Python install script (skip deps since they can be installed separately)
    run(
        [
            "sudo", sys.executable, str(SCRIPTS_DIR / "install.py"),
            "--no-deps", "--target-dir", target_dir,
        ],
        cwd=ROOT,
    )


def task_install_deps(cfg: Config) -> None:
    """Install build dependencies only."""
    run(
        ["sudo", sys.executable, str(SCRIPTS_DIR / "install.py"), "--deps-only"],
        cwd=ROOT,
    )


def task_uninstall(cfg: Config) -> None:
    """Uninstall http-tun from system."""
    run(
        ["sudo", sys.executable, str(SCRIPTS_DIR / "install.py"), "--uninstall"],
        cwd=ROOT,
    )


# =============================================================================
# Task Registry
# =============================================================================

TASKS = {
    # Development
    "dev": (task_dev, "Start development servers"),
    "build": (task_build, "Build production release"),
    "build:cli": (task_build_cli, "Build CLI tools only"),
    # Testing
    "test": (task_test, "Run all tests"),
    "test:privileged": (task_test_privileged, "Run privileged tests (requires root)"),
    "test:e2e": (task_test_e2e, "Run end-to-end tests"),
    # Code quality
    "lint": (task_lint, "Run all linters"),
    "format": (task_format, "Format all code"),
    # Installation
    "install": (task_install, "Install to system (requires sudo)"),
    "install:deps": (task_install_deps, "Install system build dependencies"),
    "uninstall": (task_uninstall, "Uninstall from system"),
    "deps:ui": (task_deps_ui, "Install UI (npm) dependencies"),
    # Maintenance
    "clean": (task_clean, "Clean build artifacts"),
    "release": (task_release, "Build release packages"),
    "debug:collect": (task_debug_collect, "Collect debug information"),
}


def print_help() -> None:
    """Print help message."""
    print(__doc__)
    print("Available tasks:")
    max_len = max(len(name) for name in TASKS)
    for name, (_, desc) in TASKS.items():
        print(f"  {name:<{max_len + 2}} {desc}")
    print()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Build task runner for http-tun",
        add_help=False,
    )
    parser.add_argument("task", nargs="?", help="Task to run")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    parser.add_argument("-h", "--help", action="store_true", help="Show help")
    parser.add_argument("--debug", action="store_true", help="Debug build (no release)")

    args = parser.parse_args()

    if args.help or not args.task:
        print_help()
        sys.exit(0)

    if args.task not in TASKS:
        print(f"❌ Unknown task: {args.task}")
        print("   Run './tasks.py --help' for available tasks")
        sys.exit(1)

    cfg = Config(
        verbose=args.verbose,
        release=not args.debug,
    )

    task_fn, _ = TASKS[args.task]
    try:
        task_fn(cfg)
    except KeyboardInterrupt:
        print("\n\n⚠️  Interrupted")
        sys.exit(130)


if __name__ == "__main__":
    main()
