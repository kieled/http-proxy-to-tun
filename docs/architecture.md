# Architecture (work in progress)

## Goals

- Remove sing-box dependency by handling TUN and proxying in-process.
- Avoid external privileged helpers (ip) in the main path. Firewall uses
  libnftnl with a fallback to `nft`/`iptables` binaries.
- Keep iptables as a fallback backend for systems without nftables.
- Support TCP over HTTP CONNECT via an upstream HTTP proxy.
- Keep behavior compatible with existing CLI flags where practical.

## Non-goals

- Full UDP tunneling. UDP is either blocked (killswitch ON) or bypassed
  directly (killswitch OFF). DNS can be handled by bypassing resolvers.
- Cross-platform support. Linux only.

## Privilege model

- The proxyvpn binary runs with CAP_NET_ADMIN (setcap once).
- No sudo required per run.
- CLI fallbacks (`nft`/`iptables`) are only attempted when running as root; native libnftnl works with CAP_NET_ADMIN.

## High-level data flow

1) Create and configure a TUN interface.
2) Add policy routes and rules so TCP flows go to the TUN table (fwmark rule uses a full mask to avoid matching unmarked traffic).
3) Read packets from TUN and feed them to a userspace TCP/IP stack.
4) For each TCP stream, open an HTTP CONNECT tunnel to the upstream proxy.
   Proxy connections are excluded from marking by destination IP.
5) Relay bytes between the local TCP stream and the CONNECT tunnel.
6) Apply firewall killswitch rules when enabled.
   Firewall allows proxy-marked TCP so output can proceed even if oifname is unset at the filter hook.

## UDP handling

- Killswitch ON: drop UDP except DNS (UDP/53) to allowed resolvers.
- Killswitch OFF: allow UDP to bypass to the main routing table.
- DNS bypass: allow UDP/53 (and TCP/53) to resolvers specified by
  --allow-dns or /etc/resolv.conf.
  If /etc/resolv.conf is empty or only contains a loopback stub, the code attempts
  /run/systemd/resolve/resolv.conf for upstreams; otherwise the allowlist is empty.

## Dependencies

- tokio: async runtime
- tun: TUN device creation + async read/write
- smoltcp: userspace TCP/IP stack
- rtnetlink: routing, link, and rule management
- netlink-packet-route: netlink types needed by rtnetlink
- nftables: managed via libnftnl (iptables fallback kept)

## Modules (proposed)

- app: runtime orchestration (CLI → setup → tunstack → teardown)
- tunstack: TUN setup + packet pump + TCP stack
- netlink: route/rule management (policy routing + bypass rules)
- firewall: nftables backend + iptables fallback
- mark: packet mark rules (nft/iptables)
- mark rules apply to all TCP (except excluded IPs) and set the proxy mark.
- proxy: HTTP CONNECT client + stream relay
- state: persisted runtime state and teardown
- util: command runner, DNS helpers, permissions, capability checks

## Compatibility notes

- Existing --dns/--allow-dns behavior is preserved via bypass rules.
- The state directory should default to $XDG_RUNTIME_DIR/proxyvpn when not root.
- iptables remains available for systems without nftables, but is still a
  privileged external dependency.

## Testing strategy

- Unit tests: pure parsing, command planning, packet parsing, and state IO.
- Integration/privileged tests: gated behind `privileged-tests` feature and
  `#[ignore]`. Use `scripts/test-privileged.sh` to run in a network namespace
  with explicit environment opt-in.
- Scripts: `scripts/test-unit.sh` (default) and `scripts/test-privileged.sh`.
