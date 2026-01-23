# app crate

Role: Orchestrates CLI → setup → TUN stack → teardown. It is the main runtime glue.

Key flow (run_up):
- Validate OS + permissions + deps.
- Parse proxy config and resolve proxy IPs.
- Resolve state dir and create state/lock.
- Create TUN device and ensure CIDR doesn't overlap existing addresses.
- Apply policy routing (mark + fwmark rule + bypass rules).
- Optionally apply firewall killswitch.
- Run TUN stack (proxy sockets are marked to bypass routing marks); on shutdown, teardown state.

Key modules:
- `config.rs`: parsing + DNS allowlist + CIDR helpers.
- `ops.rs`: trait adapters for netlink/firewall/mark/state store.
- `run.rs`: main orchestration + apply_up.
- `teardown.rs`: cleanup guard + teardown logic.
- `tun.rs`: TUN creation + overlap checks.

Tests:
- `apply_up` unit test with mock netlink/firewall/mark/store.
- config parsing + preference selection tests.
- DNS allowlist fallback test for systemd stub resolv.conf.
- firewall DNS allowlist application test (killswitch enabled).
