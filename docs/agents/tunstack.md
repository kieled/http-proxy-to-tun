# tunstack crate

Role: Userspace TCP/IP stack with smoltcp + TUN packet pump.

Key API:
- `run_tun_stack(tun_dev, cfg, shutdown)` — runs the packet loop and proxy relay.
  `TunStackConfig` carries an optional `proxy_socket_mark` (SO_MARK on proxy sockets; routing bypass is handled by proxy IP exclusions).

Module layout:
- `conn.rs`: connection state and proxy task spawn.
- `device.rs`: in-memory queue device used by smoltcp.
- `packet.rs`: packet parsing helpers (SYN sniffing).
- `stack.rs`: main loop and interface setup.

Tests:
- SYN sniffing tests.
- Queue device RX/TX behavior.
