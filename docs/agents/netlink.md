# netlink crate

Role: Netlink route/rule management and IPv4 address discovery.

Key APIs:
- `ipv4_addrs`: list IPv4 addresses on the host.
- `add_default_route_to_table`: add a default route in a specific table.
- `add_rule_fwmark_table`: policy routing by fwmark (explicit mask applied).
- `add_rule_to_ip`: bypass routing for specific destination IPs.
- `delete_rule_pref` / `delete_routes_in_table`: teardown helpers.

Tests:
- Unit tests for `route_table_id` handling.
- Privileged test for address listing (ignored by default). Set `PROXYVPN_PRIV_TESTS_REQUIRE_ADDRS=1` to assert non-empty.
