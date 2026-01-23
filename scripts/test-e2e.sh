#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${PROXYVPN_E2E_PROXY_URL:-}" ]]; then
  echo "PROXYVPN_E2E_PROXY_URL is required" >&2
  exit 1
fi

STATE_DIR=${PROXYVPN_E2E_STATE_DIR:-"/tmp/proxyvpn-e2e"}
TUN_NAME=${PROXYVPN_E2E_TUN_NAME:-"tun-e2e"}
TUN_CIDR=${PROXYVPN_E2E_TUN_CIDR:-"10.254.254.1/30"}
DNS_NAME=${PROXYVPN_E2E_DNS_NAME:-"ifconfig.me"}
DNS_SERVER=${PROXYVPN_E2E_DNS_SERVER:-"1.1.1.1"}
CURL_URL=${PROXYVPN_E2E_CURL_URL:-""}
NO_KILLSWITCH=${PROXYVPN_E2E_NO_KILLSWITCH:-"0"}
ALLOW_DNS=${PROXYVPN_E2E_ALLOW_DNS:-""}
SELFTEST_USE_PROXY=${PROXYVPN_E2E_SELFTEST_USE_PROXY:-"1"}
SELFTEST_PROXY_URL=${PROXYVPN_E2E_SELFTEST_PROXY_URL:-""}
PROXY_URL_RESOLVED=${PROXYVPN_E2E_PROXY_URL_RESOLVED:-""}
USE_RESOLVED_PROXY=${PROXYVPN_E2E_USE_RESOLVED_PROXY:-"1"}
SELFTEST_STRICT=${PROXYVPN_E2E_SELFTEST_STRICT:-"1"}
SELFTEST_SOCKET_MARK=${PROXYVPN_E2E_SELFTEST_SOCKET_MARK:-"2"}
PROXY_IP=""

resolve_proxy_url() {
  python3 - <<'PY' "$1"
import sys, urllib.parse, socket
url = sys.argv[1]
parsed = urllib.parse.urlparse(url)
if parsed.scheme != "http":
    print(url)
    raise SystemExit(0)
host = parsed.hostname
port = parsed.port
user = parsed.username
pwd = parsed.password
if not host or not port:
    print(url)
    raise SystemExit(0)
ip = host
try:
    ip = socket.gethostbyname(host)
except Exception:
    pass
auth = ""
if user:
    auth += user
if pwd:
    auth += ":" + pwd
if auth:
    auth += "@"
netloc = f"{auth}{ip}:{port}"
print(f"{parsed.scheme}://{netloc}")
PY
}

if [[ $EUID -ne 0 ]]; then
  exec sudo -E "$0" "$@"
fi

cargo build -p proxyvpn -p proxyvpn-selftest --release

cleanup() {
  ./target/release/proxyvpn down --state-dir "$STATE_DIR" --keep-logs >/dev/null 2>&1 || true
  if [[ -n "${PROXYVPN_PID:-}" ]]; then
    kill -INT "$PROXYVPN_PID" >/dev/null 2>&1 || true
    wait "$PROXYVPN_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

mkdir -p "$STATE_DIR"

if [[ "$USE_RESOLVED_PROXY" == "1" ]]; then
  if [[ -z "$PROXY_URL_RESOLVED" ]]; then
    PROXY_URL_RESOLVED="$(resolve_proxy_url "$PROXYVPN_E2E_PROXY_URL")"
  fi
else
  PROXY_URL_RESOLVED="$PROXYVPN_E2E_PROXY_URL"
fi

if [[ -z "$SELFTEST_PROXY_URL" ]]; then
  SELFTEST_PROXY_URL="$PROXY_URL_RESOLVED"
fi

PROXY_IP="$(python3 - <<'PY' "$PROXY_URL_RESOLVED"
import sys, urllib.parse
url=sys.argv[1]
p=urllib.parse.urlparse(url)
print(p.hostname or "")
PY
)"

ARGS=(
  "--proxy-url" "$PROXY_URL_RESOLVED"
  "--state-dir" "$STATE_DIR"
  "--tun-name" "$TUN_NAME"
  "--tun-cidr" "$TUN_CIDR"
  "--verbose"
)
if [[ -n "$PROXY_IP" ]]; then
  ARGS+=("--proxy-ip" "$PROXY_IP")
fi
if [[ "$NO_KILLSWITCH" == "1" ]]; then
  ARGS+=("--no-killswitch")
fi
if [[ -n "$ALLOW_DNS" ]]; then
  ARGS+=("--allow-dns" "$ALLOW_DNS")
fi

./target/release/proxyvpn "${ARGS[@]}" &
PROXYVPN_PID=$!

sleep 2

SELFTEST_ARGS=(--server "$DNS_SERVER" --name "$DNS_NAME" --no-ip)
if [[ "$SELFTEST_STRICT" == "1" ]]; then
  SELFTEST_ARGS+=(--strict)
fi
if [[ "$SELFTEST_USE_PROXY" == "1" ]]; then
  if [[ -z "$SELFTEST_PROXY_URL" ]]; then
    echo "SELFTEST_PROXY_URL is empty; cannot run DNS-over-proxy" >&2
    exit 1
  fi
  SELFTEST_ARGS+=(--proxy-url "$SELFTEST_PROXY_URL")
  if [[ -n "$SELFTEST_SOCKET_MARK" ]]; then
    SELFTEST_ARGS+=(--socket-mark "$SELFTEST_SOCKET_MARK")
  fi
fi
if ! ./target/release/proxyvpn-selftest "${SELFTEST_ARGS[@]}"; then
  echo "selftest failed; dumping routing and firewall state" >&2
  ip -4 rule show || true
  ip -4 route show table 100 || true
  if [[ -n "$PROXY_IP" ]]; then
    ip -4 route get "$PROXY_IP" || true
  fi
  nft list table inet proxyvpn || true
  nft list table inet proxyvpn_mark || true
  exit 1
fi

if [[ -n "$CURL_URL" ]]; then
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$CURL_URL" >/dev/null
  else
    echo "curl not found; skipping curl check" >&2
  fi
fi

cleanup
