use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};

use anyhow::{Context, Result, anyhow};
use proxyvpn_cli::{RunArgs, read_password};
use proxyvpn_proxy::ProxyConfig;
use proxyvpn_util::dns;

pub fn resolve_proxy_ips(host: &str, port: u16, overrides: &[IpAddr]) -> Result<Vec<IpAddr>> {
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

pub fn parse_proxy_config(args: &RunArgs) -> Result<ProxyConfig> {
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
    let password = read_password(args)?;
    Ok(ProxyConfig {
        host,
        port,
        username,
        password,
    })
}

pub fn parse_proxy_url(raw: &str) -> Result<ProxyConfig> {
    ProxyConfig::from_http_url(raw)
}

pub fn resolve_dns_allow(args: &RunArgs) -> Result<Vec<IpAddr>> {
    let resolv = dns::parse_resolv_conf("/etc/resolv.conf");
    let systemd = dns::parse_resolv_conf("/run/systemd/resolve/resolv.conf");
    resolve_dns_allow_from(args, &resolv, &systemd)
}

fn resolve_dns_allow_from(
    args: &RunArgs,
    resolv: &[IpAddr],
    systemd: &[IpAddr],
) -> Result<Vec<IpAddr>> {
    let mut ips = Vec::new();
    if !args.allow_dns.is_empty() {
        ips.extend_from_slice(&args.allow_dns);
    } else {
        ips.extend_from_slice(resolv);
        let only_loopback = !ips.is_empty() && ips.iter().all(dns::is_loopback);
        if (ips.is_empty() || only_loopback) && !systemd.is_empty() {
            ips.extend_from_slice(systemd);
        }
    }
    if let Some(dns) = args.dns {
        ips.insert(0, dns);
    }
    Ok(dedup_ips(&ips))
}

pub async fn add_bypass_rules<N: super::ops::NetlinkOps>(
    netlink: &N,
    prefs: &mut HashSet<u32>,
    start_pref: u32,
    ips: &[IpAddr],
) -> Result<Vec<proxyvpn_state::RouteBypassRule>> {
    let mut rules = Vec::new();
    let mut next = start_pref;
    for ip in ips.iter().filter(|ip| ip.is_ipv4()) {
        let pref = next_pref(prefs, next);
        let ip_v4 = match ip {
            IpAddr::V4(v4) => *v4,
            IpAddr::V6(_) => continue,
        };
        netlink.add_rule_to_ip(pref, ip_v4, 254).await?;
        rules.push(proxyvpn_state::RouteBypassRule { pref, ip: *ip });
        next = pref + 1;
    }
    Ok(rules)
}

pub fn dedup_ips(ips: &[IpAddr]) -> Vec<IpAddr> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for ip in ips {
        if seen.insert(*ip) {
            out.push(*ip);
        }
    }
    out
}

pub fn next_pref(prefs: &mut HashSet<u32>, mut pref: u32) -> u32 {
    while prefs.contains(&pref) {
        pref += 1;
    }
    prefs.insert(pref);
    pref
}

pub fn parse_tun_cidr(cidr: &str) -> Result<(Ipv4Addr, Ipv4Addr, u8)> {
    let (addr_str, prefix_str) = cidr
        .split_once('/')
        .ok_or_else(|| anyhow!("invalid TUN CIDR (expected A.B.C.D/NN)"))?;
    let addr: Ipv4Addr = addr_str.parse().context("invalid TUN IP")?;
    let prefix: u8 = prefix_str
        .parse()
        .context("invalid TUN prefix length")?;
    if prefix > 32 {
        return Err(anyhow!("invalid TUN prefix length"));
    }
    let mask = if prefix == 0 { 0u32 } else { u32::MAX << (32 - prefix) };
    let netmask = Ipv4Addr::from(mask);
    Ok((addr, netmask, prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn parse_proxy_url_requires_http() {
        let err = parse_proxy_url("https://user:pass@example.com:8080").unwrap_err();
        assert!(err.to_string().contains("http"));
    }

    #[test]
    fn parse_proxy_url_missing_user() {
        let err = parse_proxy_url("http://example.com:8080").unwrap_err();
        assert!(err.to_string().contains("username"));
    }

    #[test]
    fn parse_proxy_url_ok() {
        let cfg = parse_proxy_url("http://user:pass@example.com:8080").unwrap();
        assert_eq!(cfg.host, "example.com");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.username, "user");
        assert_eq!(cfg.password, "pass");
    }

    #[test]
    fn parse_tun_cidr_rejects_invalid_prefix() {
        let err = parse_tun_cidr("10.0.0.1/99").unwrap_err();
        assert!(err.to_string().contains("prefix"));
    }

    #[test]
    fn next_pref_skips_existing() {
        let mut prefs = HashSet::from([100, 101]);
        let next = next_pref(&mut prefs, 100);
        assert_eq!(next, 102);
        assert!(prefs.contains(&102));
    }

    fn base_args() -> RunArgs {
        RunArgs {
            state_dir: std::path::PathBuf::from("/tmp/proxyvpn-test"),
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
            allow_dns: vec![],
            no_killswitch: true,
        }
    }

    #[test]
    fn dns_allow_uses_systemd_when_stub_only() {
        let mut args = base_args();
        args.allow_dns.clear();
        let resolv = vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 53))];
        let systemd = vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))];
        let out = resolve_dns_allow_from(&args, &resolv, &systemd).unwrap();
        assert!(out.contains(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn dns_allow_uses_systemd_when_resolv_empty() {
        let args = base_args();
        let resolv = Vec::<IpAddr>::new();
        let systemd = vec![IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9))];
        let out = resolve_dns_allow_from(&args, &resolv, &systemd).unwrap();
        assert!(out.contains(&IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9))));
    }
}
