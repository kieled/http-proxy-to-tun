# mark crate

Role: Apply packet marks to route TCP via policy rules (nft/iptables).

Key APIs:
- `choose_mark_backend` picks nft or iptables based on availability.
- `remove_mark_rules_best_effort` attempts cleanup.
- Mark rules support exclusion list (proxy IPs) to avoid marking proxy traffic.
- Mark rules apply to all TCP traffic (except excluded IPs) and set the proxy mark (overwrites existing marks).
- CLI fallback runs only when root; CAP_NET_ADMIN is enough for native libnftnl.

Implementation layout:
- `nft.rs`: CLI command plan builder for nft.
- `iptables.rs`: CLI command plan builder for iptables.
- `lib.rs`: backend selection and shared types.

Tests:
- Unit tests for command plans.
- Privileged integration test (ignored by default).
