#!/usr/bin/env python3
"""
Collect debug information for proxyvpn troubleshooting.

Creates a diagnostic report with:
- System information
- Network configuration
- DNS resolution tests
- Routing tables and firewall rules
- proxyvpn state

Usage:
    sudo python3 scripts/collect_debug.py
"""

import os
import shutil
import signal
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Optional

# Output directory
TIMESTAMP = datetime.now().strftime("%Y%m%d-%H%M%S")
OUT_DIR = Path(f"/tmp/proxyvpn-debug-{TIMESTAMP}")
LOG_FILE = OUT_DIR / "diagnostics.txt"


def ensure_root() -> None:
    """Ensure running as root."""
    if os.geteuid() != 0:
        print("🔐 Elevating to root...")
        os.execvp("sudo", ["sudo", "-E", sys.executable] + sys.argv)


def run_cmd(cmd: list[str], log: Path) -> None:
    """Run command and append output to log file."""
    with open(log, "a") as f:
        f.write(f"## {' '.join(cmd)}\n")
        result = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT)
        if result.returncode != 0:
            f.write(f"(command failed: {result.returncode})\n")
        f.write("\n")


def get_state_dirs() -> list[Path]:
    """Get potential proxyvpn state directories."""
    dirs = []

    state_dir = os.environ.get("STATE_DIR")
    if state_dir:
        dirs.append(Path(state_dir))

    xdg = os.environ.get("XDG_RUNTIME_DIR")
    if xdg:
        dirs.append(Path(xdg) / "proxyvpn")

    dirs.extend([
        Path("/run/proxyvpn"),
        Path(f"/tmp/proxyvpn-{os.getuid()}"),
    ])

    return dirs


def clean_state() -> None:
    """Clean proxyvpn state directories."""
    for state_dir in get_state_dirs():
        if state_dir.exists():
            print(f"→ Removing {state_dir}")
            shutil.rmtree(state_dir)


class TcpdumpCapture:
    """Context manager for tcpdump DNS capture."""

    def __init__(self, out_dir: Path):
        self.pcap_file = out_dir / "dns.pcap"
        self.process: Optional[subprocess.Popen] = None

    def __enter__(self) -> "TcpdumpCapture":
        if not shutil.which("tcpdump"):
            return self

        self.process = subprocess.Popen(
            ["tcpdump", "-i", "any", "-n", "-w", str(self.pcap_file), "port 53 or port 853"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        print(f"→ tcpdump running (PID {self.process.pid})")
        return self

    def __exit__(self, *args) -> None:
        if self.process:
            self.process.send_signal(signal.SIGTERM)
            self.process.wait()

    def dump_to_log(self, log: Path) -> None:
        """Parse pcap and append to log."""
        if not self.pcap_file.exists():
            return

        run_cmd(["tcpdump", "-n", "-tttt", "-vvv", "-r", str(self.pcap_file)], log)


def dns_probe_script() -> str:
    """Return inline Python script for DNS probe."""
    return '''
import random, socket, struct, time

def build_query(name):
    tid = random.randint(0, 0xffff)
    flags = 0x0100  # recursion desired
    header = struct.pack("!HHHHHH", tid, flags, 1, 0, 0, 0)
    parts = name.rstrip(".").split(".")
    qname = b"".join(struct.pack("B", len(p)) + p.encode() for p in parts) + b"\\x00"
    return tid, header + qname + struct.pack("!HH", 1, 1)

def probe(server, name):
    tid, pkt = build_query(name)
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(2.0)
    start = time.time()
    try:
        s.sendto(pkt, (server, 53))
        data, _ = s.recvfrom(512)
    except Exception as e:
        return False, f"error: {e}"
    finally:
        s.close()
    if len(data) < 12:
        return False, "short response"
    rid, flags = struct.unpack("!HH", data[:4])
    rcode = flags & 0x000f
    elapsed = (time.time() - start) * 1000.0
    ok = (rid == tid and rcode == 0)
    return ok, f"id={rid} rcode={rcode} time_ms={elapsed:.1f}"

server = "192.168.0.1"
name = "ifconfig.me"
ok, msg = probe(server, name)
print(f"dns_probe {server} {name}: {'OK' if ok else 'FAIL'} {msg}")
'''


def collect_diagnostics(log: Path) -> None:
    """Collect all diagnostic information."""

    # Header
    with open(log, "w") as f:
        f.write("proxyvpn debug capture\n")
        f.write(f"Generated: {datetime.now().isoformat()}\n\n")

    # Basic system info
    print("→ Collecting system info...")
    run_cmd(["date"], log)
    run_cmd(["uname", "-a"], log)
    run_cmd(["id"], log)

    # DNS configuration
    print("→ Collecting DNS configuration...")
    run_cmd(["ls", "-l", "/etc/resolv.conf"], log)
    run_cmd(["cat", "/etc/resolv.conf"], log)
    run_cmd(["cat", "/etc/nsswitch.conf"], log)
    run_cmd(["cat", "/run/systemd/resolve/resolv.conf"], log)
    run_cmd(["cat", "/run/systemd/resolve/stub-resolv.conf"], log)

    if shutil.which("resolvectl"):
        run_cmd(["resolvectl", "dns"], log)
        run_cmd(["resolvectl", "status"], log)

    # DNS resolution tests
    print("→ Testing DNS resolution...")
    run_cmd(["getent", "hosts", "ifconfig.me"], log)

    if shutil.which("dig"):
        run_cmd(["dig", "+time=2", "+tries=1", "@192.168.0.1", "ifconfig.me"], log)

    if shutil.which("nslookup"):
        run_cmd(["nslookup", "ifconfig.me", "192.168.0.1"], log)

    # Python DNS probe
    with open(log, "a") as f:
        f.write("## Python DNS probe\n")
        result = subprocess.run(
            [sys.executable, "-c", dns_probe_script()],
            stdout=f,
            stderr=subprocess.STDOUT,
        )
        f.write("\n")

    # Network configuration
    print("→ Collecting network configuration...")
    run_cmd(["ip", "-4", "addr"], log)
    run_cmd(["ip", "-4", "link"], log)
    run_cmd(["ip", "-4", "rule", "show"], log)
    run_cmd(["ip", "-4", "route", "show"], log)
    run_cmd(["ip", "-4", "route", "show", "table", "main"], log)
    run_cmd(["ip", "-4", "route", "show", "table", "100"], log)

    # Route lookups
    for dest in ["1.1.1.1", "8.8.8.8"]:
        run_cmd(["ip", "-4", "route", "get", dest], log)
        for src in ["192.168.0.103", "10.255.255.1"]:
            run_cmd(["ip", "-4", "route", "get", dest, "from", src], log)

    # Sockets
    run_cmd(["ss", "-tupn"], log)

    # Firewall rules
    print("→ Collecting firewall rules...")
    run_cmd(["nft", "list", "table", "inet", "proxyvpn"], log)
    run_cmd(["nft", "list", "table", "inet", "proxyvpn_mark"], log)
    run_cmd(["nft", "list", "ruleset"], log)
    run_cmd(["iptables", "-S"], log)
    run_cmd(["iptables", "-t", "mangle", "-S"], log)

    # proxyvpn state
    print("→ Collecting proxyvpn state...")
    for state_dir in get_state_dirs():
        state_file = state_dir / "state.json"
        if state_file.exists():
            run_cmd(["cat", str(state_file)], log)
            break


def main() -> None:
    ensure_root()

    # Handle --clean flag
    if "--clean" in sys.argv or os.environ.get("CLEAN_STATE") == "1":
        clean_state()
        if "--clean" in sys.argv and len(sys.argv) == 2:
            print("✅ State cleaned")
            return

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    print("🔍 Proxyvpn Debug Collector")
    print(f"   Output: {LOG_FILE}")
    print()

    with TcpdumpCapture(OUT_DIR) as tcpdump:
        print("→ Run proxyvpn in another terminal, reproduce the issue,")
        print("  then press Enter to capture diagnostics...")
        input()

        collect_diagnostics(LOG_FILE)
        tcpdump.dump_to_log(LOG_FILE)

    # Copy to clipboard if possible
    if shutil.which("wl-copy"):
        with open(LOG_FILE) as f:
            subprocess.run(["wl-copy"], stdin=f)
        print("\n✅ Diagnostics copied to clipboard (wl-copy)")
    elif shutil.which("xclip"):
        with open(LOG_FILE) as f:
            subprocess.run(["xclip", "-selection", "clipboard"], stdin=f)
        print("\n✅ Diagnostics copied to clipboard (xclip)")
    else:
        print(f"\n✅ Diagnostics saved to: {LOG_FILE}")


if __name__ == "__main__":
    main()
