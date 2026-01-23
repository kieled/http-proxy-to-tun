#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  exec sudo -E "$0" "$@"
fi

ts="$(date +%Y%m%d-%H%M%S)"
out_dir="/tmp/proxyvpn-debug-$ts"
mkdir -p "$out_dir"
log="$out_dir/diagnostics.txt"

if [[ "${CLEAN_STATE:-0}" == "1" ]]; then
  state_dirs=()
  if [[ -n "${STATE_DIR:-}" ]]; then
    state_dirs+=("$STATE_DIR")
  fi
  if [[ -n "${XDG_RUNTIME_DIR:-}" ]]; then
    state_dirs+=("$XDG_RUNTIME_DIR/proxyvpn")
  fi
  state_dirs+=("/run/proxyvpn" "/tmp/proxyvpn-$(id -u)")
  for dir in "${state_dirs[@]}"; do
    if [[ -d "$dir" ]]; then
      rm -rf "$dir"
    fi
  done
fi

cat >"$log" <<'EOF'
proxyvpn debug capture
EOF

tcpdump_pid=""
if command -v tcpdump >/dev/null 2>&1; then
  tcpdump -i any -n -w "$out_dir/dns.pcap" 'port 53 or port 853' >/dev/null 2>&1 &
  tcpdump_pid="$!"
fi

echo "Output file: $log"
echo "Run proxyvpn in another terminal, reproduce the curl error, then return here."
if [[ -n "$tcpdump_pid" ]]; then
  echo "tcpdump is running (DNS capture) as PID $tcpdump_pid"
fi
read -r -n 1 -s -p "Press any key to capture diagnostics..." _
echo

run_cmd() {
  echo "## $*" >>"$log"
  if "$@" >>"$log" 2>&1; then
    true
  else
    echo "(command failed: $?)" >>"$log"
  fi
  echo >>"$log"
}

run_cmd date
run_cmd uname -a
run_cmd id
run_cmd pwd

run_cmd ls -l /etc/resolv.conf
run_cmd cat /etc/resolv.conf
run_cmd cat /etc/nsswitch.conf
run_cmd cat /run/systemd/resolve/resolv.conf
run_cmd cat /run/systemd/resolve/stub-resolv.conf
run_cmd resolvectl dns
run_cmd systemd-resolve --status
run_cmd getent hosts ifconfig.me
if command -v dig >/dev/null 2>&1; then
  run_cmd dig +time=2 +tries=1 @192.168.0.1 ifconfig.me
fi
if command -v nslookup >/dev/null 2>&1; then
  run_cmd nslookup ifconfig.me 192.168.0.1
fi
if command -v python3 >/dev/null 2>&1; then
  run_cmd python3 - <<'PY'
import os, random, socket, struct, sys, time

def build_query(name):
    tid = random.randint(0, 0xffff)
    flags = 0x0100  # recursion desired
    qdcount = 1
    header = struct.pack("!HHHHHH", tid, flags, qdcount, 0, 0, 0)
    parts = name.rstrip(".").split(".")
    qname = b"".join(struct.pack("B", len(p)) + p.encode("ascii") for p in parts) + b"\x00"
    qtype = 1  # A
    qclass = 1 # IN
    return tid, header + qname + struct.pack("!HH", qtype, qclass)

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
    rid, flags, qd, an, ns, ar = struct.unpack("!HHHHHH", data[:12])
    rcode = flags & 0x000f
    elapsed = (time.time() - start) * 1000.0
    ok = (rid == tid and rcode == 0)
    return ok, f"id={rid} rcode={rcode} an={an} time_ms={elapsed:.1f}"

server = "192.168.0.1"
name = "ifconfig.me"
ok, msg = probe(server, name)
print(f"dns_probe {server} {name}: {'OK' if ok else 'FAIL'} {msg}")
PY
fi

run_cmd ip -4 addr
run_cmd ip -4 link
run_cmd ip -4 rule show
run_cmd ip -4 route show
run_cmd ip -4 route show table main
run_cmd ip -4 route show table 100
run_cmd ip -4 route get 1.1.1.1
run_cmd ip -4 route get 1.1.1.1 from 192.168.0.103
run_cmd ip -4 route get 1.1.1.1 from 10.255.255.1
run_cmd ip -4 route get 8.8.8.8
run_cmd ip -4 route get 8.8.8.8 from 192.168.0.103
run_cmd ip -4 route get 8.8.8.8 from 10.255.255.1

# ps output removed to keep capture minimal
run_cmd ss -tupn

run_cmd nft list table inet proxyvpn
run_cmd nft list table inet proxyvpn_mark
run_cmd nft list ruleset
run_cmd iptables -S
run_cmd iptables -t mangle -S
run_cmd iptables -t mangle -S PROXYVPN_MARK
run_cmd iptables -S OUTPUT
run_cmd iptables -S PROXYVPN

state_candidates=()
if [[ -n "${STATE_DIR:-}" ]]; then
  state_candidates+=("$STATE_DIR")
fi
if [[ -n "${XDG_RUNTIME_DIR:-}" ]]; then
  state_candidates+=("$XDG_RUNTIME_DIR/proxyvpn")
fi
state_candidates+=("/run/proxyvpn" "/tmp/proxyvpn-$(id -u)")

for dir in "${state_candidates[@]}"; do
  if [[ -f "$dir/state.json" ]]; then
    run_cmd cat "$dir/state.json"
    break
  fi
done

if [[ -n "$tcpdump_pid" ]]; then
  kill "$tcpdump_pid" >/dev/null 2>&1 || true
  sleep 1
  run_cmd tcpdump -n -tttt -vvv -r "$out_dir/dns.pcap"
fi

if command -v wl-copy >/dev/null 2>&1; then
  wl-copy <"$log"
  echo "Copied diagnostics to clipboard with wl-copy."
else
  echo "wl-copy not found; diagnostics saved at $log"
fi
