# selftest crate

Role: Standalone DNS/routing probe tool.

Key APIs:
- `dns_probe`, `build_query`, `parse_response`.

Notes:
- Uses util DNS helpers to pick resolver if none provided.
- Optional IP rule/route dumps for debugging.
- Supports DNS-over-TCP via HTTP CONNECT when `--proxy-url` is provided.
  Use `--socket-mark` to set SO_MARK on the proxy TCP socket (helps bypass policy routing marks).
  Use `--no-ip` to skip `ip` command output when running as non-root.

Tests:
- Query building and response parsing.
- DNS-over-TCP via proxy test (connect + framed DNS).
- Privileged binary smoke test (ignored by default, env-gated).

End-to-end:
- `scripts/test-e2e.sh` runs proxyvpn + selftest to validate DNS while proxying.
  By default it runs DNS-over-proxy; set `PROXYVPN_E2E_SELFTEST_USE_PROXY=0` to force UDP.
  If the proxy URL uses a hostname, the script resolves it to an IP for both proxyvpn and selftest.
  On failure it dumps `ip` rules/routes and nft tables for debugging.
  `PROXYVPN_E2E_SELFTEST_STRICT=1` makes the test fail on probe errors.
  `PROXYVPN_E2E_SELFTEST_SOCKET_MARK=2` sets the proxy socket mark used by selftest.
