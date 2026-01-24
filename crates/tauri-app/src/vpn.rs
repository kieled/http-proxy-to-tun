//! VPN connection management using proxyvpn-app internals.

use std::net::{IpAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use nix::unistd::Uid;
use tokio::sync::oneshot;

use proxyvpn_firewall::{FirewallBackendKind, FirewallConfig, IptablesBackend, NftBackend};
use proxyvpn_mark::choose_mark_backend;
use proxyvpn_netlink::Netlink;
use proxyvpn_proxy::ProxyConfig;
use proxyvpn_state::{NewStateParams, State, StateStore, new_state_template};
use proxyvpn_tunstack::{TunStackConfig, run_tun_stack};
use proxyvpn_util::{CommandRunner, find_in_path};

use proxyvpn_app::ops::{FirewallOps, MarkOps, NetlinkOps, RealFirewall, RealMark, StateStoreOps};
use proxyvpn_app::teardown::teardown;
use proxyvpn_app::tun::{create_tun_device, ensure_tun_cidr_free};
use proxyvpn_app::config::{add_bypass_rules, next_pref, parse_tun_cidr};

const PROXY_BYPASS_MARK: u32 = 0x2;
const TUN_NAME: &str = "htun0";
const TUN_CIDR: &str = "10.255.255.1/30";

/// Parameters for starting a VPN connection.
pub struct VpnParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub killswitch: bool,
}

/// Run the VPN connection until shutdown signal is received.
pub async fn run_vpn(params: VpnParams, shutdown_rx: oneshot::Receiver<()>) -> Result<()> {
    let proxy = ProxyConfig {
        host: params.host.clone(),
        port: params.port,
        username: params.username,
        password: params.password,
    };

    let proxy_ips = resolve_proxy_ips(&proxy.host, proxy.port)?;
    let state_dir = resolve_state_dir();

    let runner = CommandRunner::new(false, false);
    let firewall_backend = choose_firewall_backend()?;
    let mark_backend = choose_mark_backend()?;
    let netlink = Arc::new(Netlink::new()?);

    let store = StateStore::new(state_dir.clone());
    store.ensure_dir()?;
    store.create_lock()?;

    let (tun_ip, tun_netmask, tun_prefix) = parse_tun_cidr(TUN_CIDR)?;
    ensure_tun_cidr_free(&netlink, tun_ip, tun_prefix).await?;

    let tun_dev = create_tun_device(TUN_NAME, tun_ip, tun_netmask)
        .context("failed to create TUN device")?;

    let firewall = RealFirewall::new(firewall_backend, runner.clone());
    let mark = RealMark::new(mark_backend, runner.clone());

    let state = setup(
        proxy.clone(),
        proxy_ips.clone(),
        params.killswitch,
        &store,
        &netlink,
        &firewall,
        &mark,
    )
    .await?;

    let (stack_shutdown_tx, stack_shutdown_rx) = oneshot::channel();
    let stack_cfg = TunStackConfig {
        tun_ip,
        tun_prefix,
        proxy,
        proxy_socket_mark: Some(PROXY_BYPASS_MARK),
    };
    let mut stack_task = tokio::spawn(run_tun_stack(tun_dev, stack_cfg, stack_shutdown_rx));

    // Wait for shutdown signal or stack error
    tokio::select! {
        _ = shutdown_rx => {
            let _ = stack_shutdown_tx.send(());
            let _ = stack_task.await;
        }
        res = &mut stack_task => {
            if let Err(err) = res {
                eprintln!("tun stack task error: {err}");
            }
        }
    }

    // Clean up all resources
    teardown(&state, &store, &netlink, &firewall, &mark, false).await?;
    Ok(())
}

async fn setup<N, F, M, S>(
    proxy: ProxyConfig,
    proxy_ips: Vec<IpAddr>,
    killswitch: bool,
    store: &S,
    netlink: &N,
    firewall: &F,
    mark: &M,
) -> Result<State>
where
    N: NetlinkOps,
    F: FirewallOps,
    M: MarkOps,
    S: StateStoreOps,
{
    let proxy_table = 100;
    let mut state = new_state_template(NewStateParams {
        state_dir: store.state_dir(),
        lock_path: store.lock_path(),
        tun_name: TUN_NAME.to_string(),
        tun_cidr: TUN_CIDR.to_string(),
        proxy_host: proxy.host.clone(),
        proxy_port: proxy.port,
        proxy_ips: proxy_ips.clone(),
        dns: None,
        killswitch,
        keep_logs: false,
        proxy_table,
    });

    let (tun_ip, _tun_netmask, _tun_prefix) = parse_tun_cidr(&state.tun_cidr)?;

    netlink
        .add_default_route_to_table(state.tun_name.clone(), tun_ip, proxy_table)
        .await?;

    let dns_allow = resolve_dns_allow()?;

    let mut prefs = netlink.existing_rule_prefs().await?;
    let tcp_pref = next_pref(&mut prefs, 1000);
    let mark_value = 0x1;

    // Exclude both proxy IPs and DNS IPs from marking
    let mut exclude_ips = state.proxy_ips.clone();
    exclude_ips.extend(dns_allow.iter().copied());
    mark.apply(mark_value, exclude_ips)?;

    netlink
        .add_rule_fwmark_table(tcp_pref, proxy_table, mark_value)
        .await?;
    state.tcp_rule_pref = Some(tcp_pref);
    store.write_state(&state)?;

    let dns_bypass = add_bypass_rules(netlink, &mut prefs, 200, &dns_allow).await?;
    let proxy_bypass = add_bypass_rules(netlink, &mut prefs, 300, &state.proxy_ips).await?;
    state.dns_bypass_rules = dns_bypass;
    state.proxy_bypass_rules = proxy_bypass;
    store.write_state(&state)?;

    if state.killswitch {
        let fw_state = firewall.apply(&FirewallConfig {
            tun_name: &state.tun_name,
            proxy_ips: &state.proxy_ips,
            proxy_port: state.proxy_port,
            dns_ips: &dns_allow,
            allow_udp_dns: !dns_allow.is_empty(),
            proxy_mark: mark_value,
        })?;
        state.firewall = Some(fw_state);
        store.write_state(&state)?;
    }

    Ok(state)
}

fn resolve_proxy_ips(host: &str, port: u16) -> Result<Vec<IpAddr>> {
    let mut addrs = Vec::new();
    for addr in (host, port).to_socket_addrs()? {
        addrs.push(addr.ip());
    }
    if addrs.is_empty() {
        return Err(anyhow!("proxy host did not resolve to any IPs"));
    }
    Ok(dedup_ips(&addrs))
}

fn resolve_dns_allow() -> Result<Vec<IpAddr>> {
    use proxyvpn_util::dns;
    let resolv = dns::parse_resolv_conf("/etc/resolv.conf");
    let systemd = dns::parse_resolv_conf("/run/systemd/resolve/resolv.conf");

    let mut ips = Vec::new();
    ips.extend_from_slice(&resolv);
    let only_loopback = !ips.is_empty() && ips.iter().all(dns::is_loopback);
    if (ips.is_empty() || only_loopback) && !systemd.is_empty() {
        ips.extend_from_slice(&systemd);
    }
    Ok(dedup_ips(&ips))
}

fn dedup_ips(ips: &[IpAddr]) -> Vec<IpAddr> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for ip in ips {
        if seen.insert(*ip) {
            out.push(*ip);
        }
    }
    out
}

fn choose_firewall_backend() -> Result<FirewallBackendKind> {
    if find_in_path("nft").is_some() {
        Ok(FirewallBackendKind::Nft(NftBackend {
            table: "proxyvpn".to_string(),
            chain: "output".to_string(),
        }))
    } else if find_in_path("iptables").is_some() {
        Ok(FirewallBackendKind::Iptables(IptablesBackend {
            chain: "PROXYVPN".to_string(),
        }))
    } else {
        Err(anyhow!("no firewall backend available"))
    }
}

fn resolve_state_dir() -> PathBuf {
    let default = PathBuf::from("/run/proxyvpn");
    if !Uid::effective().is_root() {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(dir).join("proxyvpn");
        }
        return std::env::temp_dir().join(format!("proxyvpn-{}", Uid::effective()));
    }
    default
}
