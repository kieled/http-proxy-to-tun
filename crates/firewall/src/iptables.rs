use anyhow::Result;

use proxyvpn_util::CommandRunner;

use crate::FirewallConfig;

pub(crate) struct IptablesPlan {
    pub(crate) cleanup: Vec<Vec<String>>,
    pub(crate) setup: Vec<Vec<String>>,
}

pub(crate) fn build_plan(cfg: &FirewallConfig, chain: &str) -> IptablesPlan {
    let mut cleanup = Vec::new();
    cleanup.push(vec!["-D", "OUTPUT", "-j", chain].into_iter().map(String::from).collect());
    cleanup.push(vec!["-F", chain].into_iter().map(String::from).collect());
    cleanup.push(vec!["-X", chain].into_iter().map(String::from).collect());

    let mut setup = Vec::new();
    setup.push(vec!["-N", chain].into_iter().map(String::from).collect());
    setup.push(vec!["-A", chain, "-o", "lo", "-j", "ACCEPT"].into_iter().map(String::from).collect());
    setup.push(vec!["-A", chain, "-o", cfg.tun_name, "-j", "ACCEPT"].into_iter().map(String::from).collect());
    setup.push(vec![
        "-A".to_string(),
        chain.to_string(),
        "-m".to_string(),
        "mark".to_string(),
        "--mark".to_string(),
        format!("0x{:x}", cfg.proxy_mark),
        "-j".to_string(),
        "ACCEPT".to_string(),
    ]);

    for ip in cfg.proxy_ips {
        let port = cfg.proxy_port.to_string();
        setup.push(vec![
            "-A".to_string(),
            chain.to_string(),
            "-p".to_string(),
            "tcp".to_string(),
            "-d".to_string(),
            ip.to_string(),
            "--dport".to_string(),
            port,
            "-j".to_string(),
            "ACCEPT".to_string(),
        ]);
    }

    for ip in cfg.dns_ips.iter().filter(|ip| ip.is_ipv4()) {
        if cfg.allow_udp_dns {
            setup.push(vec![
                "-A".to_string(),
                chain.to_string(),
                "-p".to_string(),
                "udp".to_string(),
                "-d".to_string(),
                ip.to_string(),
                "--dport".to_string(),
                "53".to_string(),
                "-j".to_string(),
                "ACCEPT".to_string(),
            ]);
        }
        setup.push(vec![
            "-A".to_string(),
            chain.to_string(),
            "-p".to_string(),
            "tcp".to_string(),
            "-d".to_string(),
            ip.to_string(),
            "--dport".to_string(),
            "53".to_string(),
            "-j".to_string(),
            "ACCEPT".to_string(),
        ]);
    }

    setup.push(vec!["-A", chain, "-j", "DROP"].into_iter().map(String::from).collect());
    setup.push(vec!["-I", "OUTPUT", "1", "-j", chain].into_iter().map(String::from).collect());

    IptablesPlan { cleanup, setup }
}

pub(crate) fn apply(cfg: &FirewallConfig, chain: &str, runner: &CommandRunner) -> Result<()> {
    let plan = build_plan(cfg, chain);
    for cmd in plan.cleanup {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        let _ = runner.run_capture_allow_fail("iptables", &args);
    }
    for cmd in plan.setup {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        runner.run("iptables", &args)?;
    }
    Ok(())
}

pub(crate) fn remove(chain: &str, runner: &CommandRunner) -> Result<()> {
    let _ = runner.run_capture_allow_fail("iptables", &["-D", "OUTPUT", "-j", chain]);
    let _ = runner.run_capture_allow_fail("iptables", &["-F", chain]);
    let _ = runner.run_capture_allow_fail("iptables", &["-X", chain]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn build_plan_orders_core_rules() {
        let cfg = FirewallConfig {
            tun_name: "tun0",
            proxy_ips: &[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))],
            proxy_port: 8080,
            dns_ips: &[IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
            allow_udp_dns: true,
            proxy_mark: 0x1,
        };
        let plan = build_plan(&cfg, "PROXYVPN");
        assert_eq!(plan.cleanup.len(), 3);
        assert_eq!(plan.setup.first().unwrap()[0], "-N");
        assert!(plan.setup.iter().any(|cmd| cmd.contains(&"-j".to_string()) && cmd.contains(&"DROP".to_string())));
        assert!(plan.setup.iter().any(|cmd| cmd.contains(&"--dport".to_string()) && cmd.contains(&"8080".to_string())));
        assert!(plan.setup.iter().any(|cmd| cmd.contains(&"--mark".to_string()) && cmd.contains(&"0x1".to_string())));
    }
}
