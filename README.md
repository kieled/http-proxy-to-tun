# proxyvpn

System-wide TCP proxy via TUN device and HTTP CONNECT protocol. Routes all TCP traffic through an upstream HTTP proxy without requiring root privileges at runtime.

## Features

- **TUN-based routing**: Creates a virtual network interface to intercept TCP traffic
- **HTTP CONNECT tunneling**: Proxies TCP connections through any HTTP CONNECT-capable proxy
- **No root at runtime**: Uses `CAP_NET_ADMIN` capability (set once during install)
- **Firewall killswitch**: Prevents traffic leaks when proxy is unavailable (default: enabled)
- **DNS bypass**: Automatically allows DNS queries to configured resolvers
- **nftables/iptables support**: Uses nftables with iptables fallback
- **Clean teardown**: Properly removes routes, rules, and firewall entries on exit

## How It Works

```
┌─────────────────────────────────────────────────────────────┐
│                        Application                          │
│                     (e.g., curl, browser)                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ TCP SYN
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      nftables/iptables                      │
│              (mark TCP packets with fwmark)                 │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ fwmark → policy route
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       TUN Device                            │
│                        (tun0)                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ raw IP packets
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    proxyvpn (userspace)                     │
├─────────────────────────────────────────────────────────────┤
│  smoltcp TCP/IP stack  ←→  HTTP CONNECT proxy client        │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ SO_MARK (bypass fwmark rule)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Upstream HTTP Proxy                     │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                         Internet
```

1. Creates a TUN interface and configures policy routing
2. Marks TCP packets with nftables/iptables to route through the TUN
3. Reads packets from TUN and processes them with a userspace TCP/IP stack (smoltcp)
4. For each TCP connection, opens an HTTP CONNECT tunnel to the upstream proxy
5. Relays data between the local TCP stream and the proxy tunnel
6. Proxy connections use socket marking (`SO_MARK`) to bypass the TUN routing

### UDP Handling

HTTP CONNECT only supports TCP. UDP traffic is handled as follows:

- **Killswitch ON** (default): UDP is blocked except DNS (port 53) to allowed resolvers
- **Killswitch OFF**: UDP bypasses the proxy and uses normal routing

## Requirements

### System

- **Linux** (kernel 3.17+ for nftables, or any kernel with iptables)
- **nftables** (preferred) or **iptables** for packet marking
- **libcap** for setting capabilities (`setcap` command)
- **iproute2** for route/rule management (`ip` command)

### Build Dependencies

- **Rust toolchain** 1.85+ (edition 2024)
- **C compiler** (gcc or clang)
- **pkg-config**
- **libnftnl-dev** / **libnftnl** (for nftables bindings)
- **libmnl-dev** / **libmnl** (netlink library)

### Package Installation

**Arch Linux:**
```bash
sudo pacman -S nftables libcap iproute2
# Build deps (usually already installed)
sudo pacman -S rust gcc pkgconf libnftnl libmnl
```

**Debian/Ubuntu:**
```bash
sudo apt install nftables libcap2-bin iproute2
# Build deps
sudo apt install rustc cargo gcc pkg-config libnftnl-dev libmnl-dev
```

**Fedora:**
```bash
sudo dnf install nftables libcap iproute
# Build deps
sudo dnf install rust cargo gcc pkg-config libnftnl-devel libmnl-devel
```

## Installation

### Quick Install (install.sh)

```bash
# Build and install to /usr/local/bin with CAP_NET_ADMIN
sudo ./install.sh

# Custom install directory
sudo INSTALL_DIR=/opt/bin ./install.sh

# Build only (no install)
./install.sh build

# Uninstall
sudo ./install.sh uninstall
```

### Arch Linux (PKGBUILD)

For local development builds:

```bash
cd pkg
makepkg -si
```

The package automatically sets `CAP_NET_ADMIN` on install via the install hook.

### Manual Installation

```bash
# Build
cargo build --release -p proxyvpn

# Install binary
sudo install -Dm755 target/release/proxyvpn /usr/local/bin/proxyvpn

# Set capability (required for non-root operation)
sudo setcap 'cap_net_admin=eip' /usr/local/bin/proxyvpn
```

If you don't want to setcap, run with sudo instead.

Note: the killswitch uses libnftnl when available, with a fallback to the
`nft`/`iptables` binaries. If the fallback path is used and you want the
killswitch enabled without sudo, you must setcap the relevant binary, or run
with `--no-killswitch`.

## Usage

### Basic Usage

Start (default subcommand is `up`) with a single proxy URL:

```bash
proxyvpn --proxy-url "http://alice:secret@192.0.2.10:8080"
```

Using separate arguments:

```bash
proxyvpn up \
  --proxy-host proxy.example.com \
  --proxy-port 8080 \
  --username alice \
  --password-file /path/to/secret \
  --tun-name tun0 \
  --tun-cidr 10.255.255.1/30
```

Stop:

```bash
proxyvpn down
```

Or press Ctrl+C in the running process.

### CLI Options

```
proxyvpn up [OPTIONS]

Proxy Configuration:
  --proxy-url <URL>         Full proxy URL: http://user:pass@host:port
  --proxy-host <HOST>       Upstream proxy hostname
  --proxy-port <PORT>       Upstream proxy port
  --username <USER>         Username for proxy auth
  --password <PASS>         Password (prefer --password-file)
  --password-file <PATH>    Read password from file
  --proxy-ip <IP>           Explicit proxy IP (skip DNS resolution, repeatable)

Network Configuration:
  --tun-name <NAME>         TUN interface name [default: tun0]
  --tun-cidr <CIDR>         TUN interface CIDR [default: 10.255.255.1/30]
  --dns <IP>                DNS IP to bypass
  --allow-dns <IP>          Additional DNS IPs to allow (repeatable)
  --no-killswitch           Disable firewall killswitch

General:
  --state-dir <PATH>        State directory [default: /run/proxyvpn]
  --verbose                 Verbose logging
  --dry-run                 Print changes without applying
  --keep-logs               Keep state files on teardown
```

### Examples

```bash
# With custom DNS servers
proxyvpn --proxy-url http://user:pass@proxy:8080 \
         --allow-dns 8.8.8.8 --allow-dns 8.8.4.4

# Without killswitch (allow direct connections when proxy fails)
proxyvpn --proxy-url http://proxy:8080 --no-killswitch

# Verbose mode for debugging
proxyvpn --proxy-url http://proxy:8080 --verbose

# Dry run to see what would be configured
proxyvpn --proxy-url http://proxy:8080 --dry-run
```

## Self-test (DNS + routing)

Run a quick DNS probe and dump the current routing rules:

```bash
cargo run -p proxyvpn-selftest -- --name ifconfig.me
```

You can override the resolver and skip `ip` output:

```bash
./target/release/proxyvpn-selftest --name ifconfig.me --server 192.168.0.1
./target/release/proxyvpn-selftest --no-ip
```

DNS-over-TCP via proxy (helps debug DNS when UDP is blocked):

```bash
./target/release/proxyvpn-selftest --proxy-url http://user:pass@proxy:3128 --server 1.1.1.1
```

Selftest flags summary:

- `--name <HOST>`: DNS name to query (default: `ifconfig.me`)
- `--server <IPv4>`: DNS server IP (defaults to first IPv4 nameserver in `/etc/resolv.conf`)
- `--timeout-ms <MS>`: probe timeout (default: 1500)
- `--no-ip`: skip `ip` command output
- `--proxy-url <URL>`: perform DNS-over-TCP via HTTP CONNECT
- `--socket-mark <MARK>`: set SO_MARK on proxy TCP socket (Linux only)

## Troubleshooting

### DNS not resolving

1. Check if DNS servers are in the bypass list:
   ```bash
   proxyvpn --verbose --proxy-url ...
   # Look for "DNS bypass IPs: [...]"
   ```

2. Add DNS servers explicitly:
   ```bash
   proxyvpn --allow-dns 8.8.8.8 --proxy-url ...
   ```

3. Verify routing rules:
   ```bash
   ip rule list
   ip route show table 100
   ```

4. If `/etc/resolv.conf` only contains a loopback stub (e.g. `127.0.0.53`) and no upstreams are available in `/run/systemd/resolve/resolv.conf`, you must pass `--allow-dns` or DNS will be blocked when the killswitch is enabled.

### Connection timeouts

1. Verify proxy is reachable:
   ```bash
   curl -x http://proxy:port http://example.com
   ```

2. Check if proxy IP is excluded from marking:
   ```bash
   nft list ruleset | grep proxyvpn
   ```

### Permission denied

Ensure the capability is set:
```bash
getcap /usr/local/bin/proxyvpn
# Should show: /usr/local/bin/proxyvpn cap_net_admin=eip
```

Re-apply if needed:
```bash
sudo setcap 'cap_net_admin=eip' /usr/local/bin/proxyvpn
```

### Clean up after crash

```bash
proxyvpn down
# Or manually:
ip link del tun0 2>/dev/null
ip rule del priority 100 2>/dev/null
ip rule del priority 200 2>/dev/null
nft delete table inet proxyvpn 2>/dev/null
```

## Architecture

### Crate Structure

| Crate | Description |
|-------|-------------|
| `proxyvpn` | Main binary entry point |
| `proxyvpn-app` | Application orchestration and runtime |
| `proxyvpn-cli` | CLI argument parsing |
| `proxyvpn-tunstack` | TUN device + smoltcp TCP/IP stack |
| `proxyvpn-proxy` | HTTP CONNECT client |
| `proxyvpn-netlink` | Route and rule management via rtnetlink |
| `proxyvpn-firewall` | Killswitch firewall rules |
| `proxyvpn-mark` | Packet marking (nft/iptables) |
| `proxyvpn-state` | Runtime state persistence |
| `proxyvpn-util` | DNS helpers and utilities |
| `proxyvpn-selftest` | DNS and routing diagnostic tool |

### Key Dependencies

- **tokio**: async runtime
- **tun**: TUN device creation + async read/write
- **smoltcp**: userspace TCP/IP stack
- **rtnetlink**: routing, link, and rule management
- **nftnl/mnl**: nftables rule management via libnftnl

## Notes / Limitations

- Only TCP is proxied. UDP is blocked when the killswitch is enabled, except for DNS (UDP/53) to the allowed resolvers. With killswitch disabled, UDP bypasses directly.
- HTTPS traffic is tunneled using HTTP CONNECT via the upstream proxy. No TLS decryption is performed.
- DNS is not automatically reconfigured on the host; use `--dns`/`--allow-dns` to allow resolvers and ensure your system resolver is configured appropriately.
- IPv4 is supported; IPv6 is not fully handled in firewall rules.
- Killswitch rules are scoped to a dedicated nftables table (`inet proxyvpn`) or an iptables chain (`PROXYVPN`).

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release -p proxyvpn
```

### Tests

- Unit tests: `scripts/test-unit.sh`
- Privileged tests (root + netns recommended): `scripts/test-privileged.sh`
- End-to-end DNS test (requires proxy URL): `scripts/test-e2e.sh`

E2E environment variables:
- `PROXYVPN_E2E_PROXY_URL=http://user:pass@proxy:3128` (required)
- `PROXYVPN_E2E_PROXY_URL_RESOLVED=http://user:pass@1.2.3.4:3128` (optional override)
- `PROXYVPN_E2E_USE_RESOLVED_PROXY=1` (default: resolve hostname to IP before running)
- `PROXYVPN_E2E_DNS_SERVER=1.1.1.1`
- `PROXYVPN_E2E_DNS_NAME=ifconfig.me`
- `PROXYVPN_E2E_ALLOW_DNS=1.1.1.1` (use UDP DNS with killswitch enabled)
- `PROXYVPN_E2E_SELFTEST_USE_PROXY=1` (default: DNS-over-proxy selftest)
- `PROXYVPN_E2E_SELFTEST_PROXY_URL=http://user:pass@proxy:3128` (optional override)
- `PROXYVPN_E2E_SELFTEST_STRICT=1` (default: fail e2e if selftest fails)
- `PROXYVPN_E2E_SELFTEST_SOCKET_MARK=2` (default: set SO_MARK on proxy TCP in selftest)

### Developer docs

- Agent-oriented docs live under `docs/agents/`.
- If you change code/tests/architecture, update `docs/agents/` and `docs/architecture.md`.

## License

MIT
