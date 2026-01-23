# Agent Docs Overview

These docs are for AI agents and maintainers to quickly understand the project layout,
responsibilities, and test strategy. Keep them in sync with any changes.

## Start here
- `docs/agents/crates.md` — concise purpose + key APIs/tests for every crate.
- `docs/architecture.md` — high‑level design and data flow.
- Per‑crate docs: `docs/agents/{app,cli,firewall,mark,netlink,proxy,proxyvpn,state,tunstack,util,selftest}.md`

## Working rules
- If you touch a crate, update its entry in `docs/agents/crates.md`.
- If you change behavior/flow, update `docs/architecture.md`.
- If you add/remove tests or scripts, update the testing section below.

## Testing (current)
- Unit tests cover parsing, command plans, packet parsing, and state IO.
- Privileged tests are gated behind the `privileged-tests` feature and `#[ignore]`.
- Use `scripts/test-unit.sh` for default coverage.
- Use `scripts/test-privileged.sh` for root/netns-gated tests (requires explicit env flags).
- Use `scripts/test-e2e.sh` for end-to-end DNS validation via proxy (requires proxy URL).

### Privileged test env flags
- `PROXYVPN_PRIV_TESTS_ALLOW_FIREWALL=1`
- `PROXYVPN_PRIV_TESTS_ALLOW_MARK=1`
- `PROXYVPN_PRIV_TESTS_ALLOW_NETLINK=1`
- `PROXYVPN_PRIV_TESTS_REQUIRE_ADDRS=1` (netlink test will fail if no IPv4 addrs)
- `PROXYVPN_PRIV_TESTS_ALLOW_DNS=1`
- `PROXYVPN_PRIV_TESTS_DNS_SERVER=1.1.1.1`
- `PROXYVPN_PRIV_TESTS_DNS_NAME=ifconfig.me`
- `PROXYVPN_PRIV_TESTS_PROXY_URL=http://user:pass@proxy:3128`
- `PROXYVPN_E2E_PROXY_URL=http://user:pass@proxy:3128`
- `PROXYVPN_E2E_PROXY_URL_RESOLVED=http://user:pass@1.2.3.4:3128` (optional override)
- `PROXYVPN_E2E_USE_RESOLVED_PROXY=1` (default: resolve hostname to IP before running)
- `PROXYVPN_E2E_DNS_SERVER=1.1.1.1`
- `PROXYVPN_E2E_DNS_NAME=ifconfig.me`
- `PROXYVPN_E2E_CURL_URL=https://ifconfig.me`
- `PROXYVPN_E2E_NO_KILLSWITCH=1` (disable killswitch for e2e)
- `PROXYVPN_E2E_ALLOW_DNS=1.1.1.1` (allow UDP DNS when killswitch is enabled)
- `PROXYVPN_E2E_SELFTEST_USE_PROXY=1` (default: use DNS-over-proxy in e2e)
- `PROXYVPN_E2E_SELFTEST_PROXY_URL=http://user:pass@proxy:3128` (optional override; if unset, script resolves proxy host to IP)
- `PROXYVPN_E2E_PROXY_URL_RESOLVED=http://user:pass@1.2.3.4:3128` (used for both proxyvpn and selftest)
- `PROXYVPN_E2E_SELFTEST_STRICT=1` (default: fail e2e if selftest fails)
- `PROXYVPN_E2E_SELFTEST_SOCKET_MARK=2` (default: set SO_MARK on proxy TCP in selftest)

## Key entrypoints
- `crates/proxyvpn/src/main.rs` — binary entrypoint.
- `crates/app/src/lib.rs` — main runtime orchestration.
- `crates/tunstack/src/lib.rs` — userspace TCP/IP stack + TUN IO.
