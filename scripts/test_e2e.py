#!/usr/bin/env python3
"""
End-to-end test runner for proxyvpn.

Starts proxyvpn with the specified configuration, runs selftest to verify
DNS resolution works through the proxy, then cleans up.

Environment variables:
    PROXYVPN_E2E_PROXY_URL      Required. Proxy URL (http://user:pass@host:port)
    PROXYVPN_E2E_STATE_DIR      State directory (default: /tmp/proxyvpn-e2e)
    PROXYVPN_E2E_TUN_NAME       TUN interface name (default: tun-e2e)
    PROXYVPN_E2E_TUN_CIDR       TUN CIDR (default: 10.254.254.1/30)
    PROXYVPN_E2E_DNS_NAME       DNS name to test (default: ifconfig.me)
    PROXYVPN_E2E_DNS_SERVER     DNS server (default: 1.1.1.1)
    PROXYVPN_E2E_NO_KILLSWITCH  Disable killswitch (default: 0)
    PROXYVPN_E2E_CURL_URL       Optional URL to curl after test
"""

import os
import shutil
import signal
import socket
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional
from urllib.parse import urlparse

ROOT = Path(__file__).parent.parent.resolve()
TARGET = ROOT / "target" / "release"


@dataclass
class E2EConfig:
    """E2E test configuration from environment."""
    proxy_url: str
    state_dir: Path = Path("/tmp/proxyvpn-e2e")
    tun_name: str = "tun-e2e"
    tun_cidr: str = "10.254.254.1/30"
    dns_name: str = "ifconfig.me"
    dns_server: str = "1.1.1.1"
    no_killswitch: bool = False
    allow_dns: str = ""
    curl_url: str = ""
    selftest_use_proxy: bool = True
    selftest_strict: bool = True
    selftest_socket_mark: str = "2"

    # Computed fields
    proxy_url_resolved: str = field(init=False)
    proxy_ip: str = field(init=False)

    def __post_init__(self) -> None:
        self.proxy_url_resolved = self._resolve_proxy_url(self.proxy_url)
        self.proxy_ip = self._extract_host(self.proxy_url_resolved)

    @staticmethod
    def from_env() -> "E2EConfig":
        """Create config from environment variables."""
        proxy_url = os.environ.get("PROXYVPN_E2E_PROXY_URL")
        if not proxy_url:
            print("❌ PROXYVPN_E2E_PROXY_URL is required", file=sys.stderr)
            sys.exit(1)

        def env_bool(key: str, default: str = "0") -> bool:
            return os.environ.get(key, default) == "1"

        return E2EConfig(
            proxy_url=proxy_url,
            state_dir=Path(os.environ.get("PROXYVPN_E2E_STATE_DIR", "/tmp/proxyvpn-e2e")),
            tun_name=os.environ.get("PROXYVPN_E2E_TUN_NAME", "tun-e2e"),
            tun_cidr=os.environ.get("PROXYVPN_E2E_TUN_CIDR", "10.254.254.1/30"),
            dns_name=os.environ.get("PROXYVPN_E2E_DNS_NAME", "ifconfig.me"),
            dns_server=os.environ.get("PROXYVPN_E2E_DNS_SERVER", "1.1.1.1"),
            no_killswitch=env_bool("PROXYVPN_E2E_NO_KILLSWITCH"),
            allow_dns=os.environ.get("PROXYVPN_E2E_ALLOW_DNS", ""),
            curl_url=os.environ.get("PROXYVPN_E2E_CURL_URL", ""),
            selftest_use_proxy=env_bool("PROXYVPN_E2E_SELFTEST_USE_PROXY", "1"),
            selftest_strict=env_bool("PROXYVPN_E2E_SELFTEST_STRICT", "1"),
            selftest_socket_mark=os.environ.get("PROXYVPN_E2E_SELFTEST_SOCKET_MARK", "2"),
        )

    @staticmethod
    def _resolve_proxy_url(url: str) -> str:
        """Resolve proxy hostname to IP address."""
        parsed = urlparse(url)
        if parsed.scheme != "http":
            return url

        host = parsed.hostname
        port = parsed.port
        if not host or not port:
            return url

        # Try to resolve hostname
        try:
            ip = socket.gethostbyname(host)
        except socket.gaierror:
            ip = host

        # Rebuild URL with resolved IP
        auth = ""
        if parsed.username:
            auth = parsed.username
            if parsed.password:
                auth += f":{parsed.password}"
            auth += "@"

        return f"{parsed.scheme}://{auth}{ip}:{port}"

    @staticmethod
    def _extract_host(url: str) -> str:
        """Extract hostname/IP from URL."""
        parsed = urlparse(url)
        return parsed.hostname or ""


class ProxyvpnProcess:
    """Manager for proxyvpn background process."""

    def __init__(self, config: E2EConfig):
        self.config = config
        self.process: Optional[subprocess.Popen] = None

    def start(self) -> None:
        """Start proxyvpn in the background."""
        cfg = self.config
        cfg.state_dir.mkdir(parents=True, exist_ok=True)

        args = [
            str(TARGET / "proxyvpn"),
            "--proxy-url", cfg.proxy_url_resolved,
            "--state-dir", str(cfg.state_dir),
            "--tun-name", cfg.tun_name,
            "--tun-cidr", cfg.tun_cidr,
            "--verbose",
        ]

        if cfg.proxy_ip:
            args.extend(["--proxy-ip", cfg.proxy_ip])

        if cfg.no_killswitch:
            args.append("--no-killswitch")

        if cfg.allow_dns:
            args.extend(["--allow-dns", cfg.allow_dns])

        print(f"→ Starting proxyvpn: {' '.join(args)}")
        self.process = subprocess.Popen(args)

        # Wait for startup
        time.sleep(2)

        if self.process.poll() is not None:
            print("❌ proxyvpn exited unexpectedly", file=sys.stderr)
            sys.exit(1)

    def stop(self) -> None:
        """Stop proxyvpn gracefully."""
        if self.process is None:
            return

        # Send SIGINT for graceful shutdown
        self.process.send_signal(signal.SIGINT)
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()

    def __enter__(self) -> "ProxyvpnProcess":
        self.start()
        return self

    def __exit__(self, *args) -> None:
        self.stop()


def run_selftest(config: E2EConfig) -> bool:
    """Run proxyvpn-selftest and return success status."""
    args = [
        str(TARGET / "proxyvpn-selftest"),
        "--server", config.dns_server,
        "--name", config.dns_name,
        "--no-ip",
    ]

    if config.selftest_strict:
        args.append("--strict")

    if config.selftest_use_proxy:
        args.extend(["--proxy-url", config.proxy_url_resolved])
        if config.selftest_socket_mark:
            args.extend(["--socket-mark", config.selftest_socket_mark])

    print(f"→ Running selftest: {' '.join(args)}")
    result = subprocess.run(args)
    return result.returncode == 0


def dump_debug_info(config: E2EConfig) -> None:
    """Dump routing and firewall state for debugging."""
    print("\n📋 Debug information:", file=sys.stderr)

    commands = [
        ["ip", "-4", "rule", "show"],
        ["ip", "-4", "route", "show", "table", "100"],
        ["nft", "list", "table", "inet", "proxyvpn"],
        ["nft", "list", "table", "inet", "proxyvpn_mark"],
    ]

    if config.proxy_ip:
        commands.append(["ip", "-4", "route", "get", config.proxy_ip])

    for cmd in commands:
        print(f"\n$ {' '.join(cmd)}", file=sys.stderr)
        subprocess.run(cmd, stderr=subprocess.STDOUT)


def run_curl_test(url: str) -> bool:
    """Run optional curl test."""
    if not shutil.which("curl"):
        print("⚠️  curl not found; skipping curl check", file=sys.stderr)
        return True

    print(f"→ Running curl: {url}")
    result = subprocess.run(
        ["curl", "-fsSL", url],
        stdout=subprocess.DEVNULL,
    )
    return result.returncode == 0


def ensure_root() -> None:
    """Ensure running as root, re-exec with sudo if needed."""
    if os.geteuid() != 0:
        print("🔐 Elevating to root...")
        os.execvp("sudo", ["sudo", "-E", sys.executable] + sys.argv)


def build_binaries() -> None:
    """Build required binaries."""
    print("→ Building binaries...")
    subprocess.run(
        ["cargo", "build", "-p", "proxyvpn", "-p", "proxyvpn-selftest", "--release"],
        cwd=ROOT,
        check=True,
    )


def main() -> None:
    ensure_root()

    config = E2EConfig.from_env()

    print("🧪 E2E Test Runner")
    print(f"   Proxy: {config.proxy_url_resolved}")
    print(f"   State: {config.state_dir}")
    print()

    build_binaries()

    with ProxyvpnProcess(config):
        # Run selftest
        if not run_selftest(config):
            print("\n❌ Selftest failed!", file=sys.stderr)
            dump_debug_info(config)
            sys.exit(1)

        # Optional curl test
        if config.curl_url:
            if not run_curl_test(config.curl_url):
                print(f"\n❌ Curl test failed: {config.curl_url}", file=sys.stderr)
                sys.exit(1)

    print("\n✅ E2E tests passed!")


if __name__ == "__main__":
    main()
