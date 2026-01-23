use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use nix::unistd::Uid;

use proxyvpn_cli::{RunArgs, parse_cli};
use proxyvpn_firewall::{FirewallBackendKind, FirewallConfig, IptablesBackend, NftBackend};
use proxyvpn_mark::choose_mark_backend;
use proxyvpn_netlink::Netlink;
use proxyvpn_proxy::ProxyConfig;
use proxyvpn_state::{NewStateParams, State, StateStore, new_state_template};
use proxyvpn_tunstack::{TunStackConfig, run_tun_stack};
use proxyvpn_util::{CommandRunner, find_in_path, has_cap_net_admin};

use super::config::{add_bypass_rules, parse_proxy_config, parse_tun_cidr, resolve_dns_allow, resolve_proxy_ips};
use super::ops::{FirewallOps, MarkOps, NetlinkOps, RealFirewall, RealMark, StateStoreOps};
use super::teardown::teardown as do_teardown;
use super::tun::{create_tun_device, ensure_tun_cidr_free};

const PROXY_BYPASS_MARK: u32 = 0x2;

pub async fn run() -> Result<()> {
    let cli = parse_cli();
    run_with_args(cli.args).await
}

async fn run_with_args(args: RunArgs) -> Result<()> {
    ensure_linux()?;
    ensure_net_admin()?;
    ensure_deps()?;

    let proxy = parse_proxy_config(&args)?;
    let proxy_ips = resolve_proxy_ips(&proxy.host, proxy.port, &args.proxy_ip)?;
    let state_dir = resolve_state_dir(&args);

    if args.dry_run {
        print_dry_run(&args, &proxy, &proxy_ips, &state_dir);
        return Ok(());
    }

    let runner = CommandRunner::new(args.verbose, false);
    let firewall_backend = choose_firewall_backend()?;
    let mark_backend = choose_mark_backend()?;
    let netlink = Arc::new(Netlink::new()?);

    let store = StateStore::new(state_dir.clone());
    store.ensure_dir()?;
    store.create_lock()?;

    let (tun_ip, tun_netmask, tun_prefix) = parse_tun_cidr(&args.tun_cidr)?;
    ensure_tun_cidr_free(&netlink, tun_ip, tun_prefix).await?;

    let tun_dev = create_tun_device(&args.tun_name, tun_ip, tun_netmask)
        .context("failed to create TUN device")?;

    let firewall = RealFirewall::new(firewall_backend, runner.clone());
    let mark = RealMark::new(mark_backend, runner.clone());

    let state = setup(
        &args,
        proxy.clone(),
        proxy_ips.clone(),
        &store,
        &netlink,
        &firewall,
        &mark,
    )
    .await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let stack_cfg = TunStackConfig {
        tun_ip,
        tun_prefix,
        proxy,
        proxy_socket_mark: Some(PROXY_BYPASS_MARK),
    };
    let mut stack_task = tokio::spawn(run_tun_stack(tun_dev, stack_cfg, shutdown_rx));

    // Wait for shutdown signal or stack error
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nShutting down...");
            let _ = shutdown_tx.send(());
            let _ = stack_task.await;
        }
        res = &mut stack_task => {
            if let Err(err) = res {
                eprintln!("tun stack task error: {err}");
            }
        }
    }

    // Clean up all resources
    do_teardown(&state, &store, &netlink, &firewall, &mark, state.keep_logs).await?;
    Ok(())
}

async fn setup<N, F, M, S>(
    args: &RunArgs,
    proxy: ProxyConfig,
    proxy_ips: Vec<IpAddr>,
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
        tun_name: args.tun_name.clone(),
        tun_cidr: args.tun_cidr.clone(),
        proxy_host: proxy.host.clone(),
        proxy_port: proxy.port,
        proxy_ips: proxy_ips.clone(),
        dns: args.dns,
        killswitch: !args.no_killswitch,
        keep_logs: args.keep_logs || args.verbose,
        proxy_table,
    });

    let (tun_ip, _tun_netmask, _tun_prefix) = parse_tun_cidr(&state.tun_cidr)?;

    netlink
        .add_default_route_to_table(state.tun_name.clone(), tun_ip, proxy_table)
        .await?;

    let dns_allow = resolve_dns_allow(args)?;
    if args.verbose {
        eprintln!("DNS bypass IPs: {:?}", dns_allow);
    }

    let mut prefs = netlink.existing_rule_prefs().await?;
    let tcp_pref = super::config::next_pref(&mut prefs, 1000);
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

fn ensure_linux() -> Result<()> {
    if std::env::consts::OS != "linux" {
        return Err(anyhow!("this tool only supports Linux"));
    }
    Ok(())
}

fn ensure_net_admin() -> Result<()> {
    if Uid::effective().is_root() || has_cap_net_admin() {
        return Ok(());
    }
    Err(anyhow!("must be run as root or have CAP_NET_ADMIN"))
}

fn ensure_deps() -> Result<()> {
    if find_in_path("nft").is_none() && find_in_path("iptables").is_none() {
        return Err(anyhow!("missing dependency: nft or iptables"));
    }
    Ok(())
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

fn print_dry_run(
    args: &RunArgs,
    proxy: &ProxyConfig,
    proxy_ips: &[IpAddr],
    state_dir: &std::path::Path,
) {
    eprintln!(
        "dry-run: would create state dir {}",
        state_dir.display()
    );
    eprintln!(
        "dry-run: would create TUN {} ({})",
        args.tun_name, args.tun_cidr
    );
    eprintln!(
        "dry-run: would start TUN stack to {}:{} via HTTP CONNECT user {}",
        proxy.host, proxy.port, proxy.username
    );
    eprintln!("dry-run: resolved proxy IPs: {:?}", proxy_ips);
    if !args.no_killswitch {
        eprintln!("dry-run: would apply firewall killswitch");
    }
}

fn resolve_state_dir(args: &RunArgs) -> PathBuf {
    let default = PathBuf::from("/run/proxyvpn");
    if args.state_dir == default && !Uid::effective().is_root() {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(dir).join("proxyvpn");
        }
        return std::env::temp_dir().join(format!("proxyvpn-{}", Uid::effective()));
    }
    args.state_dir.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use proxyvpn_state::FirewallState;
    use std::collections::HashSet;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockNetlink {
        calls: Mutex<Vec<String>>,
        addrs: Vec<Ipv4Addr>,
        prefs: Mutex<HashSet<u32>>,
    }

    impl NetlinkOps for MockNetlink {
        async fn ipv4_addrs(&self) -> Result<Vec<Ipv4Addr>> {
            Ok(self.addrs.clone())
        }

        async fn add_default_route_to_table(
            &self,
            tun_name: String,
            tun_ip: Ipv4Addr,
            table: u32,
        ) -> Result<()> {
            self.calls
                .lock()
                .expect("calls")
                .push(format!("add_default_route {tun_name} {tun_ip} {table}"));
            Ok(())
        }

        async fn existing_rule_prefs(&self) -> Result<HashSet<u32>> {
            Ok(self.prefs.lock().expect("prefs").clone())
        }

        async fn add_rule_fwmark_table(
            &self,
            pref: u32,
            table: u32,
            mark: u32,
        ) -> Result<()> {
            self.calls.lock().expect("calls").push(format!(
                "add_rule_fwmark {pref} {table} {mark}"
            ));
            Ok(())
        }

        async fn add_rule_to_ip(&self, pref: u32, ip: Ipv4Addr, table: u32) -> Result<()> {
            self.calls.lock().expect("calls").push(format!(
                "add_rule_to_ip {pref} {ip} {table}"
            ));
            Ok(())
        }

        async fn delete_rule_pref(&self, pref: u32) -> Result<()> {
            self.calls
                .lock()
                .expect("calls")
                .push(format!("delete_rule {pref}"));
            Ok(())
        }

        async fn delete_routes_in_table(&self, table: u32) -> Result<()> {
            self.calls
                .lock()
                .expect("calls")
                .push(format!("delete_routes {table}"));
            Ok(())
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct FirewallConfigOwned {
        tun_name: String,
        proxy_ips: Vec<IpAddr>,
        proxy_port: u16,
        dns_ips: Vec<IpAddr>,
        allow_udp_dns: bool,
    }

    impl From<&FirewallConfig<'_>> for FirewallConfigOwned {
        fn from(cfg: &FirewallConfig<'_>) -> Self {
            Self {
                tun_name: cfg.tun_name.to_string(),
                proxy_ips: cfg.proxy_ips.to_vec(),
                proxy_port: cfg.proxy_port,
                dns_ips: cfg.dns_ips.to_vec(),
                allow_udp_dns: cfg.allow_udp_dns,
            }
        }
    }

    #[derive(Default)]
    struct MockFirewall {
        applied: Mutex<Vec<FirewallConfigOwned>>,
    }

    impl FirewallOps for MockFirewall {
        fn apply(&self, cfg: &FirewallConfig) -> Result<FirewallState> {
            self.applied.lock().expect("applied").push(cfg.into());
            Ok(FirewallState::Iptables {
                chain: "PROXYVPN".to_string(),
            })
        }

        fn remove_from_state(&self, _state: &FirewallState) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockMark {
        marks: Mutex<Vec<u32>>,
        exclude_ips: Mutex<Vec<IpAddr>>,
        removed: Mutex<usize>,
    }

    impl MarkOps for MockMark {
        fn apply(&self, mark: u32, exclude_ips: Vec<IpAddr>) -> Result<()> {
            self.marks.lock().expect("marks").push(mark);
            *self.exclude_ips.lock().expect("exclude_ips") = exclude_ips;
            Ok(())
        }

        fn remove_best_effort(&self) -> Result<()> {
            *self.removed.lock().expect("removed") += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockStore {
        state: Mutex<Option<State>>,
        writes: Mutex<usize>,
        state_dir: PathBuf,
        lock_path: PathBuf,
    }

    impl MockStore {
        fn new() -> Self {
            let state_dir = PathBuf::from("/tmp/proxyvpn-test");
            let lock_path = state_dir.join("lock");
            Self {
                state: Mutex::new(None),
                writes: Mutex::new(0),
                state_dir,
                lock_path,
            }
        }
    }

    impl StateStoreOps for MockStore {
        fn state_dir(&self) -> PathBuf {
            self.state_dir.clone()
        }

        fn lock_path(&self) -> PathBuf {
            self.lock_path.clone()
        }

        fn write_state(&self, state: &State) -> Result<()> {
            *self.writes.lock().expect("writes") += 1;
            *self.state.lock().expect("state") = Some(state.clone());
            Ok(())
        }

        fn remove_state_files(&self, _keep_logs: bool) -> Result<()> {
            Ok(())
        }
    }

    fn base_args() -> RunArgs {
        RunArgs {
            state_dir: PathBuf::from("/tmp/proxyvpn-test"),
            verbose: false,
            keep_logs: false,
            dry_run: false,
            proxy_url: None,
            proxy_host: Some("example.com".to_string()),
            proxy_port: Some(8080),
            username: Some("user".to_string()),
            password: Some("secret".to_string()),
            password_file: None,
            proxy_ip: Vec::new(),
            tun_name: "tun0".to_string(),
            tun_cidr: "10.255.255.1/30".to_string(),
            dns: None,
            allow_dns: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))],
            no_killswitch: true,
        }
    }

    #[tokio::test]
    async fn setup_writes_state_and_rules() {
        let args = base_args();
        let proxy = ProxyConfig {
            host: "example.com".to_string(),
            port: 8080,
            username: "user".to_string(),
            password: "secret".to_string(),
        };
        let proxy_ips = vec![IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))];
        let netlink = MockNetlink::default();
        let firewall = MockFirewall::default();
        let mark = MockMark::default();
        let store = MockStore::new();

        let state = setup(&args, proxy, proxy_ips.clone(), &store, &netlink, &firewall, &mark)
            .await
            .unwrap();

        assert_eq!(state.proxy_ips, proxy_ips);
        assert!(state.tcp_rule_pref.is_some());
        let calls = netlink.calls.lock().expect("calls").clone();
        assert!(calls.iter().any(|c| c.contains("add_default_route")));
        assert!(calls.iter().any(|c| c.contains("add_rule_fwmark")));
        assert!(calls.iter().any(|c| c.contains("add_rule_to_ip")));
    }

    #[tokio::test]
    async fn setup_applies_firewall_with_dns_allowlist() {
        let mut args = base_args();
        args.no_killswitch = false;
        args.allow_dns = vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))];
        let proxy = ProxyConfig {
            host: "example.com".to_string(),
            port: 8080,
            username: "user".to_string(),
            password: "secret".to_string(),
        };
        let proxy_ips = vec![IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))];
        let netlink = MockNetlink::default();
        let firewall = MockFirewall::default();
        let mark = MockMark::default();
        let store = MockStore::new();

        let _state = setup(&args, proxy, proxy_ips, &store, &netlink, &firewall, &mark)
            .await
            .unwrap();

        let applied = firewall.applied.lock().expect("applied").clone();
        assert_eq!(applied.len(), 1);
        assert!(applied[0].allow_udp_dns);
        assert!(applied[0].dns_ips.contains(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[tokio::test]
    async fn setup_excludes_dns_ips_from_mark() {
        let mut args = base_args();
        args.allow_dns = vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))];
        let proxy = ProxyConfig {
            host: "example.com".to_string(),
            port: 8080,
            username: "user".to_string(),
            password: "secret".to_string(),
        };
        let proxy_ips = vec![IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))];
        let netlink = MockNetlink::default();
        let firewall = MockFirewall::default();
        let mark = MockMark::default();
        let store = MockStore::new();

        let _state = setup(&args, proxy, proxy_ips.clone(), &store, &netlink, &firewall, &mark)
            .await
            .unwrap();

        let exclude_ips = mark.exclude_ips.lock().expect("exclude_ips").clone();
        assert!(exclude_ips.contains(&IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))));
        assert!(exclude_ips.contains(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }
}
