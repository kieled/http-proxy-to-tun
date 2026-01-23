#!/usr/bin/env bash
set -euo pipefail

USE_NETNS=1
if [[ -n "${PROXYVPN_E2E_PROXY_URL:-}" ]]; then
  USE_NETNS=0
fi
for arg in "$@"; do
  if [[ "$arg" == "--no-netns" ]]; then
    USE_NETNS=0
  fi
done

if [[ $USE_NETNS -eq 1 ]] && command -v unshare >/dev/null 2>&1; then
  if [[ $EUID -ne 0 ]]; then
    exec sudo -E unshare -n -- "$0" --no-netns "$@"
  else
    exec unshare -n -- "$0" --no-netns "$@"
  fi
fi

if [[ $EUID -ne 0 ]]; then
  exec sudo -E "$0" "$@"
fi

export PROXYVPN_PRIV_TESTS_ALLOW_FIREWALL=1
export PROXYVPN_PRIV_TESTS_ALLOW_MARK=1
export PROXYVPN_PRIV_TESTS_ALLOW_NETLINK=1
export PROXYVPN_PRIV_TESTS_ALLOW_DNS=1

cargo test -p proxyvpn-firewall --features privileged-tests -- --ignored
cargo test -p proxyvpn-mark --features privileged-tests -- --ignored
cargo test -p proxyvpn-netlink --features privileged-tests -- --ignored
cargo test -p proxyvpn-selftest --features privileged-tests -- --ignored

if [[ -n "${PROXYVPN_E2E_PROXY_URL:-}" ]]; then
  ./scripts/test-e2e.sh
fi
