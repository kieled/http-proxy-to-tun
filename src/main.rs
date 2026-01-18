mod cli;
mod firewall;
mod netstate;
mod singbox;
mod state;
mod util;

use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use nix::unistd::Uid;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use url::Url;

use crate::cli::{Command, DownArgs, UpArgs, parse_cli_with_default_up};
use crate::firewall::{
    FirewallBackendKind, FirewallConfig, FirewallState, IptablesBackend, NftBackend,
};
use crate::netstate::{
    delete_tun, get_default_routes, get_sysctl_value, restore_default_routes, tun_exists,
};
use crate::singbox::{SingBoxConfig, SingBoxManager};
use crate::state::{NewStateParams, RouteBypassRule, State, StateStore, new_state_template};
use crate::util::{CommandRunner, find_in_path};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {:#}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = parse_cli_with_default_up();
    match cli.command {
        Command::Up(args) => run_up(*args),
        Command::Down(args) => run_down(*args),
    }
}

fn run_up(args: UpArgs) -> Result<()> {
    ensure_linux()?;
    ensure_root()?;
    ensure_deps()?;

    let proxy = parse_proxy_config(&args)?;
    let proxy_ips = resolve_proxy_ips(&proxy.host, proxy.port, &args.proxy_ip)?;

    if args.common.dry_run {
        print_dry_run_up(&args, &proxy, &proxy_ips);
        return Ok(());
    }

    let runner = CommandRunner::new(args.common.verbose, false);
    let firewall_backend = choose_firewall_backend()?;

    let store = StateStore::new(args.common.state_dir.clone());
    store.ensure_dir()?;
    let _lock = store.create_lock()?;

    let mut state = new_state_template(NewStateParams {
        state_dir: store.state_dir.clone(),
        lock_path: store.lock_path.clone(),
        tun_name: args.tun_name.clone(),
        tun_cidr: args.tun_cidr.clone(),
        proxy_host: proxy.host.clone(),
        proxy_port: proxy.port,
        proxy_ips: proxy_ips.clone(),
        dns: args.dns,
        killswitch: !args.no_killswitch,
        keep_logs: args.common.keep_logs || args.common.verbose,
    });

    let mut guard = CleanupGuard::new(store.clone(), runner.clone(), state.clone());

    state.sysctl.ipv4_forward = get_sysctl_value(&runner, "net.ipv4.ip_forward").ok();
    state.sysctl.ipv6_forward = get_sysctl_value(&runner, "net.ipv6.conf.all.forwarding").ok();
    state.routes_before = get_default_routes(&runner)?;
    store.write_state(&state)?;
    guard.update(state.clone());

    let manager = SingBoxManager {
        config_path: state.sing_box.config_path.clone(),
        stdout_path: state.sing_box.stdout_path.clone(),
        stderr_path: state.sing_box.stderr_path.clone(),
    };

    manager.write_config(&SingBoxConfig {
        tun_name: &state.tun_name,
        tun_cidr: &state.tun_cidr,
        proxy_host: &state.proxy_host,
        proxy_port: state.proxy_port,
        username: &proxy.username,
        password: &proxy.password,
        dns: state.dns,
    })?;

    let mut child = manager.start()?;
    state.sing_box.pid = Some(child.id() as i32);
    store.write_state(&state)?;
    guard.update(state.clone());

    wait_for_tun(&runner, &state.tun_name, Duration::from_secs(10))
        .context("timed out waiting for TUN interface")?;

    let dns_allow = resolve_dns_allow(&args)?;
    if state.killswitch {
        let dns_bypass = add_bypass_rules(&runner, &dns_allow)?;
        let proxy_bypass = add_bypass_rules(&runner, &state.proxy_ips)?;
        state.dns_bypass_rules = dns_bypass;
        state.proxy_bypass_rules = proxy_bypass;
        store.write_state(&state)?;
        let fw_state = firewall_backend.apply(
            &FirewallConfig {
                tun_name: &state.tun_name,
                proxy_ips: &state.proxy_ips,
                proxy_port: state.proxy_port,
                dns_ips: &dns_allow,
            },
            &runner,
        )?;
        state.firewall = Some(fw_state);
        store.write_state(&state)?;
        guard.update(state.clone());
    }

    let runtime = Arc::new(Runtime::new(store, runner, firewall_backend));
    runtime.set_state(state);

    let (tx, rx) = mpsc::channel::<()>();
    let tx_signal = tx.clone();
    thread::spawn(move || {
        let mut signals = Signals::new([SIGINT, SIGTERM]).expect("signal handler");
        if let Some(_sig) = signals.forever().next() {
            let _ = tx_signal.send(());
        }
    });

    let tx_child = tx.clone();
    thread::spawn(move || {
        let _ = child.wait();
        let _ = tx_child.send(());
    });

    guard.disarm();
    let _ = rx.recv();
    runtime.teardown_once();
    Ok(())
}

fn run_down(args: DownArgs) -> Result<()> {
    ensure_linux()?;
    ensure_root()?;

    if args.common.dry_run {
        eprintln!(
            "dry-run: would attempt teardown in {}",
            args.common.state_dir.display()
        );
        return Ok(());
    }

    let runner = CommandRunner::new(args.common.verbose, false);
    let firewall_backend = choose_firewall_backend().unwrap_or_else(|_| {
        FirewallBackendKind::Iptables(IptablesBackend {
            chain: "PROXYVPN".to_string(),
        })
    });

    let store = StateStore::new(args.common.state_dir.clone());
    if let Ok(state) = store.read_state() {
        let keep_logs = args.common.keep_logs || state.keep_logs;
        teardown_state(&state, &store, &runner, keep_logs)?;
    } else {
        firewall_backend.remove_best_effort(&runner)?;
        store.remove_state_files(args.common.keep_logs)?;
    }
    Ok(())
}

fn ensure_linux() -> Result<()> {
    if std::env::consts::OS != "linux" {
        return Err(anyhow!("this tool only supports Linux"));
    }
    Ok(())
}

fn ensure_root() -> Result<()> {
    if !Uid::effective().is_root() {
        return Err(anyhow!("must be run as root"));
    }
    Ok(())
}

fn ensure_deps() -> Result<()> {
    if find_in_path("sing-box").is_none() {
        return Err(anyhow!("missing dependency: sing-box"));
    }
    if find_in_path("ip").is_none() {
        return Err(anyhow!("missing dependency: ip (iproute2)"));
    }
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

fn resolve_proxy_ips(host: &str, port: u16, overrides: &[IpAddr]) -> Result<Vec<IpAddr>> {
    if !overrides.is_empty() {
        return Ok(dedup_ips(overrides));
    }
    let mut addrs = Vec::new();
    for addr in (host, port).to_socket_addrs()? {
        addrs.push(addr.ip());
    }
    if addrs.is_empty() {
        return Err(anyhow!("proxy host did not resolve to any IPs"));
    }
    Ok(dedup_ips(&addrs))
}

struct ProxyConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
}

fn parse_proxy_config(args: &UpArgs) -> Result<ProxyConfig> {
    if let Some(url) = &args.proxy_url {
        return parse_proxy_url(url);
    }
    let host = args
        .proxy_host
        .as_ref()
        .cloned()
        .ok_or_else(|| anyhow!("missing --proxy-host"))?;
    let port = args
        .proxy_port
        .ok_or_else(|| anyhow!("missing --proxy-port"))?;
    let username = args
        .username
        .as_ref()
        .cloned()
        .ok_or_else(|| anyhow!("missing --username"))?;
    let password = cli::read_password(args)?;
    Ok(ProxyConfig {
        host,
        port,
        username,
        password,
    })
}

fn parse_proxy_url(raw: &str) -> Result<ProxyConfig> {
    let url = Url::parse(raw).context("invalid proxy URL")?;
    if url.scheme() != "http" {
        return Err(anyhow!("proxy URL must use http:// scheme"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("proxy URL missing host"))?
        .to_string();
    let port = url
        .port()
        .ok_or_else(|| anyhow!("proxy URL missing port"))?;
    let username = url.username();
    if username.is_empty() {
        return Err(anyhow!("proxy URL missing username"));
    }
    let password = url
        .password()
        .ok_or_else(|| anyhow!("proxy URL missing password"))?;
    Ok(ProxyConfig {
        host,
        port,
        username: username.to_string(),
        password: password.to_string(),
    })
}

fn resolve_dns_allow(args: &UpArgs) -> Result<Vec<IpAddr>> {
    if !args.allow_dns.is_empty() {
        return Ok(dedup_ips(&args.allow_dns));
    }
    let resolv = std::fs::read_to_string("/etc/resolv.conf").unwrap_or_default();
    let mut ips = Vec::new();
    for line in resolv.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("nameserver") {
            let ip_str = rest.trim();
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                ips.push(ip);
            }
        }
    }
    Ok(dedup_ips(&ips))
}

fn add_bypass_rules(runner: &CommandRunner, ips: &[IpAddr]) -> Result<Vec<RouteBypassRule>> {
    let mut prefs = existing_rule_prefs(runner)?;
    let mut next_pref: u32 = 1000;
    let mut rules = Vec::new();
    for ip in ips.iter().filter(|ip| ip.is_ipv4()) {
        while prefs.contains(&next_pref) {
            next_pref += 1;
        }
        runner.run(
            "ip",
            &[
                "rule",
                "add",
                "pref",
                &next_pref.to_string(),
                "to",
                &format!("{}/32", ip),
                "lookup",
                "main",
            ],
        )?;
        rules.push(RouteBypassRule {
            pref: next_pref,
            ip: *ip,
        });
        prefs.insert(next_pref);
        next_pref += 1;
    }
    Ok(rules)
}

fn existing_rule_prefs(runner: &CommandRunner) -> Result<std::collections::HashSet<u32>> {
    let mut prefs = std::collections::HashSet::new();
    let out = runner.run_capture("ip", &["rule", "show"])?;
    for line in out.lines() {
        if let Some((pref, _)) = line.split_once(':')
            && let Ok(num) = pref.trim().parse::<u32>()
        {
            prefs.insert(num);
        }
    }
    Ok(prefs)
}

fn dedup_ips(ips: &[IpAddr]) -> Vec<IpAddr> {
    let mut out = Vec::new();
    for ip in ips {
        if !out.contains(ip) {
            out.push(*ip);
        }
    }
    out
}

fn wait_for_tun(runner: &CommandRunner, tun_name: &str, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if tun_exists(runner, tun_name) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!("tun not ready"))
}

fn stop_process(pid: i32, timeout: Duration) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let pid_raw = pid;
    let pid = Pid::from_raw(pid_raw);
    let _ = kill(pid, Signal::SIGTERM);
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !std::path::Path::new(&format!("/proc/{}/", pid_raw)).exists() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = kill(pid, Signal::SIGKILL);
}

fn teardown_state(
    state: &State,
    store: &StateStore,
    runner: &CommandRunner,
    keep_logs: bool,
) -> Result<()> {
    if let Some(pid) = state.sing_box.pid {
        stop_process(pid, Duration::from_secs(5));
    }

    if let Some(fw) = &state.firewall {
        let _ = remove_firewall_from_state(fw, runner);
    }

    for rule in &state.dns_bypass_rules {
        let _ =
            runner.run_capture_allow_fail("ip", &["rule", "del", "pref", &rule.pref.to_string()]);
    }
    for rule in &state.proxy_bypass_rules {
        let _ =
            runner.run_capture_allow_fail("ip", &["rule", "del", "pref", &rule.pref.to_string()]);
    }

    let _ = restore_default_routes(runner, &state.routes_before);

    if state.sysctl.changed {
        if let Some(val) = &state.sysctl.ipv4_forward {
            let _ = runner.run("sysctl", &["-w", &format!("net.ipv4.ip_forward={}", val)]);
        }
        if let Some(val) = &state.sysctl.ipv6_forward {
            let _ = runner.run(
                "sysctl",
                &["-w", &format!("net.ipv6.conf.all.forwarding={}", val)],
            );
        }
    }

    if tun_exists(runner, &state.tun_name) {
        let _ = delete_tun(runner, &state.tun_name);
    }

    store.remove_state_files(keep_logs)?;
    Ok(())
}

fn remove_firewall_from_state(fw: &FirewallState, runner: &CommandRunner) -> Result<()> {
    match fw {
        FirewallState::Nft { table, .. } => {
            if find_in_path("nft").is_some() {
                let _ = runner.run_capture_allow_fail("nft", &["delete", "table", "inet", table]);
            }
        }
        FirewallState::Iptables { chain } => {
            if find_in_path("iptables").is_some() {
                let _ = runner.run_capture_allow_fail("iptables", &["-D", "OUTPUT", "-j", chain]);
                let _ = runner.run_capture_allow_fail("iptables", &["-F", chain]);
                let _ = runner.run_capture_allow_fail("iptables", &["-X", chain]);
            }
        }
    }
    Ok(())
}

fn print_dry_run_up(args: &UpArgs, proxy: &ProxyConfig, proxy_ips: &[IpAddr]) {
    eprintln!(
        "dry-run: would create state dir {}",
        args.common.state_dir.display()
    );
    eprintln!(
        "dry-run: would create TUN {} ({})",
        args.tun_name, args.tun_cidr
    );
    eprintln!(
        "dry-run: would start sing-box to {}:{} via HTTP proxy user {}",
        proxy.host, proxy.port, proxy.username
    );
    eprintln!("dry-run: resolved proxy IPs: {:?}", proxy_ips);
    if !args.no_killswitch {
        eprintln!("dry-run: would apply firewall killswitch");
    }
}

struct Runtime {
    state: Mutex<Option<State>>,
    store: StateStore,
    runner: CommandRunner,
    firewall_backend: FirewallBackendKind,
    tearing_down: std::sync::atomic::AtomicBool,
}

impl Runtime {
    fn new(
        store: StateStore,
        runner: CommandRunner,
        firewall_backend: FirewallBackendKind,
    ) -> Self {
        Self {
            state: Mutex::new(None),
            store,
            runner,
            firewall_backend,
            tearing_down: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn set_state(&self, state: State) {
        let mut guard = self.state.lock().expect("state lock");
        *guard = Some(state);
    }

    fn teardown_once(&self) {
        if self
            .tearing_down
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        let state = { self.state.lock().expect("state lock").clone() };
        if let Some(state) = state {
            let _ = teardown_state(&state, &self.store, &self.runner, state.keep_logs);
        } else {
            let _ = self.firewall_backend.remove_best_effort(&self.runner);
            let _ = self.store.remove_state_files(false);
        }
    }
}

struct CleanupGuard {
    state: Option<State>,
    store: StateStore,
    runner: CommandRunner,
}

impl CleanupGuard {
    fn new(store: StateStore, runner: CommandRunner, state: State) -> Self {
        Self {
            state: Some(state),
            store,
            runner,
        }
    }

    fn update(&mut self, state: State) {
        self.state = Some(state);
    }

    fn disarm(&mut self) {
        self.state = None;
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            let _ = teardown_state(state, &self.store, &self.runner, state.keep_logs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn dedup_ips_preserves_order() {
        let ips = vec![
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        ];
        let out = dedup_ips(&ips);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
        assert_eq!(out[1], IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)));
    }

    #[test]
    fn resolve_proxy_ips_uses_overrides() {
        let overrides = vec![IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9))];
        let out = resolve_proxy_ips("example.com", 8080, &overrides).unwrap();
        assert_eq!(out, overrides);
    }
}
