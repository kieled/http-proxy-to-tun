# Crate Map

This file is a quick per‑crate reference. Update it on any behavior or structure change.

## app
- Role: Orchestrates CLI → setup → tun stack → teardown.
- Key APIs: `run`, `run_up`, `run_down`, `apply_up`.
- Depends on: cli, firewall, mark, netlink, proxy, state, tunstack, util.
- Tests: unit tests for config parsing, overlap detection, and `apply_up` flow.
- Modules: `config`, `ops`, `run`, `teardown`, `tun`.

## cli
- Role: CLI definition and parsing helpers.
- Key APIs: `parse_cli_with_default_up`, `read_password`.
- Tests: password parsing (inline/file).

## firewall
- Role: nftables (native) + iptables fallback killswitch.
- Key APIs: `FirewallBackendKind::apply/remove_best_effort`.
- Uses libnftnl for native operations; falls back to `nft`/`iptables` CLI only when running as root.
- Notes: killswitch allows proxy-marked TCP in addition to lo/tun/dns/proxy.
- Tests: command plan builders for iptables/nft CLI.

## mark
- Role: Apply packet mark rules for TCP routing (nft/iptables).
- Key APIs: `choose_mark_backend`, `remove_mark_rules_best_effort`.
- Tests: command plan builders for iptables/nft CLI.
- Notes: mark rules exclude proxy IPs and apply to all TCP traffic (overwrites existing marks).
- CLI fallback runs only when root; CAP_NET_ADMIN is sufficient for native libnftnl.

## netlink
- Role: Netlink routes/rules and address discovery.
- Key APIs: `ipv4_addrs`, `add_rule_fwmark_table` (with mask), `delete_routes_in_table`.
- Tests: `route_table_id` unit tests; privileged smoke test for address listing.

## proxy
- Role: HTTP CONNECT handshake to upstream proxy and stream tunneling.
- Key APIs: `connect_http_connect`, `connect_http_connect_with`.
- Tests: CONNECT success and error handling.

## state
- Role: Persist runtime state on disk for teardown/recovery.
- Key APIs: `StateStore::{write_state,read_state,remove_state_files}`.
- Tests: state roundtrip + keep_logs behavior.

## tunstack
- Role: Userspace TCP/IP stack with smoltcp + TUN packet pump.
- Key APIs: `run_tun_stack`.
- Modules: `conn`, `device`, `packet`, `stack`.
- Tests: packet SYN sniffing and device queue behavior.
- Notes: `TunStackConfig.proxy_socket_mark` sets SO_MARK on proxy sockets; routing bypass is now handled by proxy IP exclusions.

## util
- Role: Command runner, PATH lookup, permissions helpers, CAP_NET_ADMIN check.
- Extras: `is_root` helper for gating privileged CLI fallbacks.
- Extras: DNS parsing helpers in `dns` module.
- Tests: DNS parsing + loopback detection.

## proxyvpn (binary)
- Role: Binary entrypoint (`main`).

## selftest
- Role: Standalone DNS/routing probe tool.
- Key APIs: `dns_probe`, `build_query`, `parse_response`.
- Extras: DNS-over-TCP via HTTP CONNECT when `--proxy-url` is set; `--socket-mark` allows SO_MARK.
- Tests: privileged binary smoke test (env-gated).
