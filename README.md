# proxyvpn

System-wide “VPN-like” proxying on Arch Linux using a TUN interface and sing-box.

## Requirements

- Arch Linux (Linux only)
- Root privileges
- `sing-box` in PATH (package: `sing-box`)
- `ip` (iproute2)
- `nft` (preferred) or `iptables`

## Build

```bash
cargo build --release
```

## Usage

Start (default subcommand is `up`) with a single proxy URL (no TUN options needed):

```bash
sudo ./target/release/proxyvpn \
  --proxy-url "http://alice:secret@192.0.2.10:8080"
```

Other usage (explicit fields and TUN options):

```bash
sudo ./target/release/proxyvpn up \
  --proxy-host proxy.example.com \
  --proxy-port 8080 \
  --username alice \
  --password-file /path/to/secret \
  --tun-name tun0 \
  --tun-cidr 172.19.0.1/30
```

Stop:

```bash
sudo ./target/release/proxyvpn down
```

### Optional flags

- `--proxy-ip <IP>` (repeatable): skip DNS resolution and use explicit proxy IPs for the killswitch.
- `--proxy-url <URL>`: single proxy URL like `http://user:pass@host:port` (mutually exclusive with host/port/user/password flags).
- `--dns <IP>`: add a DNS server to sing-box config (system resolver is not modified).
- `--allow-dns <IP>` (repeatable): allow DNS queries to these IPs when killswitch is enabled. If not set, the tool will allow resolvers from `/etc/resolv.conf` to avoid breaking DNS.
- `--no-killswitch`: disable firewall killswitch (default is ON).
- `--state-dir <PATH>`: defaults to `/run/proxyvpn`.
- `--verbose`: verbose logs.
- `--keep-logs`: keep sing-box logs after teardown (also implied by `--verbose`).

## Smoke test

1. Start proxyvpn:

   ```bash
   sudo ./target/release/proxyvpn up --proxy-host ... --proxy-port ... --username ... --password-file ...
   ```

2. Verify:

   ```bash
   curl https://example.com
   ```

3. Confirm egress only to proxy IP (example):

   ```bash
   sudo ss -tupn | rg ':<proxy_port>'
   # or
   sudo tcpdump -i <uplink> host <proxy_ip>
   ```

4. Stop:

   Press Ctrl+C in the running `proxyvpn` process, or run:

   ```bash
   sudo ./target/release/proxyvpn down
   ```

5. Confirm default routes and firewall are restored.

## Notes / limitations

- HTTPS traffic is tunneled using HTTP CONNECT via the upstream proxy. No TLS decryption is performed.
- DNS is not automatically reconfigured on the host; pass `--dns` to configure sing-box DNS if desired and ensure your system resolver is configured to use it.
- IPv4 is supported; IPv6 is not fully handled in firewall rules.
- Killswitch rules are scoped to a dedicated nftables table (`inet proxyvpn`) or an iptables chain (`PROXYVPN`).
