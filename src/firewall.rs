use std::net::IpAddr;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::util::CommandRunner;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "backend", rename_all = "snake_case")]
pub enum FirewallState {
    Nft { table: String, chain: String },
    Iptables { chain: String },
}

pub struct FirewallConfig<'a> {
    pub tun_name: &'a str,
    pub proxy_ips: &'a [IpAddr],
    pub proxy_port: u16,
    pub dns_ips: &'a [IpAddr],
}

pub trait FirewallBackend {
    fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState>;
    fn remove(&self, state: &FirewallState, runner: &CommandRunner) -> Result<()>;
}

pub struct NftBackend {
    pub table: String,
    pub chain: String,
}

impl FirewallBackend for NftBackend {
    fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState> {
        let table = &self.table;
        let chain = &self.chain;
        let _ = runner.run_capture_allow_fail("nft", &["delete", "table", "inet", table]);
        runner.run("nft", &["add", "table", "inet", table])?;
        runner.run(
            "nft",
            &[
                "add", "chain", "inet", table, chain, "{", "type", "filter", "hook", "output",
                "priority", "0", ";", "policy", "drop", ";", "}",
            ],
        )?;

        runner.run(
            "nft",
            &[
                "add", "rule", "inet", table, chain, "oifname", "lo", "accept",
            ],
        )?;
        runner.run(
            "nft",
            &[
                "add",
                "rule",
                "inet",
                table,
                chain,
                "oifname",
                cfg.tun_name,
                "accept",
            ],
        )?;
        for ip in cfg.proxy_ips {
            let ip_str = ip.to_string();
            let port = cfg.proxy_port.to_string();
            runner.run(
                "nft",
                &[
                    "add", "rule", "inet", table, chain, "ip", "daddr", &ip_str, "tcp", "dport",
                    &port, "accept",
                ],
            )?;
        }
        for ip in cfg.dns_ips.iter().filter(|ip| ip.is_ipv4()) {
            let ip_str = ip.to_string();
            runner.run(
                "nft",
                &[
                    "add", "rule", "inet", table, chain, "ip", "daddr", &ip_str, "udp", "dport",
                    "53", "accept",
                ],
            )?;
            runner.run(
                "nft",
                &[
                    "add", "rule", "inet", table, chain, "ip", "daddr", &ip_str, "tcp", "dport",
                    "53", "accept",
                ],
            )?;
        }
        Ok(FirewallState::Nft {
            table: table.clone(),
            chain: chain.clone(),
        })
    }

    fn remove(&self, state: &FirewallState, runner: &CommandRunner) -> Result<()> {
        if let FirewallState::Nft { table, .. } = state {
            let _ = runner.run_capture_allow_fail("nft", &["delete", "table", "inet", table]);
        }
        Ok(())
    }
}

pub struct IptablesBackend {
    pub chain: String,
}

impl FirewallBackend for IptablesBackend {
    fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState> {
        let chain = &self.chain;
        let _ = runner.run_capture_allow_fail("iptables", &["-D", "OUTPUT", "-j", chain]);
        let _ = runner.run_capture_allow_fail("iptables", &["-F", chain]);
        let _ = runner.run_capture_allow_fail("iptables", &["-X", chain]);
        runner.run("iptables", &["-N", chain])?;
        runner.run("iptables", &["-A", chain, "-o", "lo", "-j", "ACCEPT"])?;
        runner.run(
            "iptables",
            &["-A", chain, "-o", cfg.tun_name, "-j", "ACCEPT"],
        )?;
        for ip in cfg.proxy_ips {
            let ip_str = ip.to_string();
            let port = cfg.proxy_port.to_string();
            runner.run(
                "iptables",
                &[
                    "-A", chain, "-p", "tcp", "-d", &ip_str, "--dport", &port, "-j", "ACCEPT",
                ],
            )?;
        }
        for ip in cfg.dns_ips.iter().filter(|ip| ip.is_ipv4()) {
            let ip_str = ip.to_string();
            runner.run(
                "iptables",
                &[
                    "-A", chain, "-p", "udp", "-d", &ip_str, "--dport", "53", "-j", "ACCEPT",
                ],
            )?;
            runner.run(
                "iptables",
                &[
                    "-A", chain, "-p", "tcp", "-d", &ip_str, "--dport", "53", "-j", "ACCEPT",
                ],
            )?;
        }
        runner.run("iptables", &["-A", chain, "-j", "DROP"])?;
        runner.run("iptables", &["-I", "OUTPUT", "1", "-j", chain])?;
        Ok(FirewallState::Iptables {
            chain: chain.clone(),
        })
    }

    fn remove(&self, state: &FirewallState, runner: &CommandRunner) -> Result<()> {
        if let FirewallState::Iptables { chain } = state {
            let _ = runner.run_capture_allow_fail("iptables", &["-D", "OUTPUT", "-j", chain]);
            let _ = runner.run_capture_allow_fail("iptables", &["-F", chain]);
            let _ = runner.run_capture_allow_fail("iptables", &["-X", chain]);
        }
        Ok(())
    }
}

pub enum FirewallBackendKind {
    Nft(NftBackend),
    Iptables(IptablesBackend),
}

impl FirewallBackendKind {
    pub fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState> {
        match self {
            FirewallBackendKind::Nft(backend) => backend.apply(cfg, runner),
            FirewallBackendKind::Iptables(backend) => backend.apply(cfg, runner),
        }
    }

    pub fn remove_best_effort(&self, runner: &CommandRunner) -> Result<()> {
        match self {
            FirewallBackendKind::Nft(backend) => {
                let state = FirewallState::Nft {
                    table: backend.table.clone(),
                    chain: backend.chain.clone(),
                };
                backend.remove(&state, runner)?;
            }
            FirewallBackendKind::Iptables(backend) => {
                let state = FirewallState::Iptables {
                    chain: backend.chain.clone(),
                };
                backend.remove(&state, runner)?;
            }
        }
        Ok(())
    }
}
