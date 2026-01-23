# firewall crate

Role: Apply firewall killswitch rules with nftables (native/libnftnl) and iptables fallback.

Key concepts:
- `FirewallConfig` describes tun name, proxy IPs, DNS allowlist, ports, and the proxy mark.
- Nft backend tries native libnftnl first; CLI fallback is only attempted when running as root.
- Iptables backend uses the `iptables` CLI and therefore also requires root.

Implementation layout:
- `nft.rs`: native netlink operations + CLI command plan builder.
- `iptables.rs`: CLI command plan builder.
- `lib.rs`: backend selection + public API.

Tests:
- Unit tests validate generated CLI command plans.
- Privileged integration test exists but is ignored by default; use script + env flags.
