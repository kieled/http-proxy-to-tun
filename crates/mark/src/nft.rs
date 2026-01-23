use std::ffi::CString;
use std::net::IpAddr;

use anyhow::{Context, Result, anyhow};
use nix::libc;
use nftnl::{Batch, Chain, ChainType, FinalizedBatch, Hook, MsgType, Policy, ProtoFamily, Rule, Table, nft_expr};

use proxyvpn_util::CommandRunner;

use crate::MarkConfig;

pub(crate) fn build_commands(cfg: &MarkConfig, table: &str, chain: &str) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    cmds.push(vec!["delete", "table", "inet", table].into_iter().map(String::from).collect());
    cmds.push(vec!["add", "table", "inet", table].into_iter().map(String::from).collect());
    cmds.push(vec![
        "add", "chain", "inet", table, chain, "{", "type", "route", "hook", "output",
        "priority", "-150", ";", "policy", "accept", ";", "}",
    ].into_iter().map(String::from).collect());
    // Early accept for non-TCP traffic (UDP DNS, ICMP, etc.) to avoid any
    // interference from the type route chain processing
    cmds.push(vec![
        "add", "rule", "inet", table, chain, "meta", "l4proto", "!=", "tcp", "accept",
    ].into_iter().map(String::from).collect());
    for ip in cfg.exclude_ips.iter().filter(|ip| ip.is_ipv4()) {
        cmds.push(vec![
            "add".to_string(),
            "rule".to_string(),
            "inet".to_string(),
            table.to_string(),
            chain.to_string(),
            "ip".to_string(),
            "daddr".to_string(),
            ip.to_string(),
            "accept".to_string(),
        ]);
    }
    let mark = format!("0x{:x}", cfg.mark);
    cmds.push(vec![
        "add".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "meta".to_string(),
        "l4proto".to_string(),
        "tcp".to_string(),
        "meta".to_string(),
        "mark".to_string(),
        "set".to_string(),
        mark,
    ]);
    cmds
}

pub(crate) fn apply(cfg: &MarkConfig, table: &str, chain: &str, runner: &CommandRunner) -> Result<()> {
    if !proxyvpn_util::is_root() {
        return Err(anyhow!(
            "nft CLI requires root; CAP_NET_ADMIN on proxyvpn does not grant nft binary permissions"
        ));
    }
    let cmds = build_commands(cfg, table, chain);
    for (idx, cmd) in cmds.into_iter().enumerate() {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        if idx == 0 {
            let _ = runner.run_capture_allow_fail("nft", &args);
        } else {
            runner.run("nft", &args)?;
        }
    }
    Ok(())
}

/// Apply mark rules using native libnftnl (works with CAP_NET_ADMIN)
pub(crate) fn apply_native(cfg: &MarkConfig, table: &str, chain: &str) -> Result<()> {
    let table_c = CString::new(table).context("invalid nft table name")?;
    let chain_c = CString::new(chain).context("invalid nft chain name")?;
    let table = Table::new(table_c.as_c_str(), ProtoFamily::Inet);
    let mut chain = Chain::new(chain_c.as_c_str(), &table);
    chain.set_type(ChainType::Route);
    chain.set_hook(Hook::Out, -150);
    chain.set_policy(Policy::Accept);

    // Delete existing table first (ignore errors)
    let _ = delete_table(table_c.to_str().unwrap_or_default());

    let mut batch = Batch::new();
    batch.add(&table, MsgType::Add);
    batch.add(&chain, MsgType::Add);

    // Rule: non-TCP traffic -> accept (early return for UDP DNS, etc.)
    add_rule_non_tcp_accept(&mut batch, &chain);

    // Rules: exclude IPs -> accept
    for ip in &cfg.exclude_ips {
        if let IpAddr::V4(v4) = ip {
            add_rule_ip_accept(&mut batch, &chain, *v4);
        }
    }

    // Rule: l4proto == tcp -> set mark
    add_rule_tcp_set_mark(&mut batch, &chain, cfg.mark);

    let finalized = batch.finalize();
    send_and_process(&finalized)?;
    Ok(())
}

pub(crate) fn delete_table(table: &str) -> Result<()> {
    let table_c = CString::new(table).context("invalid nft table name")?;
    let table_obj = Table::new(table_c.as_c_str(), ProtoFamily::Inet);
    let mut batch = Batch::new();
    batch.add(&table_obj, MsgType::Del);
    let finalized = batch.finalize();
    // Ignore "table not found" errors but propagate others
    match send_and_process(&finalized) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(2) => Ok(()), // ENOENT - table doesn't exist
        Err(e) => Err(anyhow::anyhow!("failed to delete nft table {}: {}", table, e)),
    }
}

/// Accept non-TCP traffic (UDP, ICMP, etc.)
fn add_rule_non_tcp_accept(batch: &mut Batch, chain: &Chain) {
    let mut rule = Rule::new(chain);
    rule.add_expr(&nft_expr!(meta l4proto));
    rule.add_expr(&nft_expr!(cmp != libc::IPPROTO_TCP as u8));
    rule.add_expr(&nft_expr!(verdict accept));
    batch.add(&rule, MsgType::Add);
}

/// Accept traffic to a specific IPv4 address
fn add_rule_ip_accept(batch: &mut Batch, chain: &Chain, ip: std::net::Ipv4Addr) {
    let mut rule = Rule::new(chain);
    rule.add_expr(&nft_expr!(meta nfproto));
    rule.add_expr(&nft_expr!(cmp == libc::NFPROTO_IPV4 as u8));
    rule.add_expr(&nft_expr!(payload ipv4 daddr));
    rule.add_expr(&nft_expr!(cmp == ip));
    rule.add_expr(&nft_expr!(verdict accept));
    batch.add(&rule, MsgType::Add);
}

/// Set mark on TCP traffic that has mark == 0
fn add_rule_tcp_set_mark(batch: &mut Batch, chain: &Chain, mark: u32) {
    let mut rule = Rule::new(chain);
    // Check l4proto == tcp
    rule.add_expr(&nft_expr!(meta l4proto));
    rule.add_expr(&nft_expr!(cmp == libc::IPPROTO_TCP as u8));
    // Set mark
    rule.add_expr(&nft_expr!(immediate data mark));
    rule.add_expr(&nft_expr!(meta mark set));
    batch.add(&rule, MsgType::Add);
}

fn send_and_process(batch: &FinalizedBatch) -> std::io::Result<()> {
    let socket = mnl::Socket::new(mnl::Bus::Netfilter)?;
    let portid = socket.portid();
    socket.send_all(batch)?;
    let mut buffer = vec![0; nftnl::nft_nlmsg_maxsize() as usize];
    let mut expected_seqs = batch.sequence_numbers();
    while !expected_seqs.is_empty() {
        let len = socket.recv(&mut buffer[..])?;
        let expected_seq = expected_seqs.next().expect("unexpected nft ack");
        mnl::cb_run(&buffer[..len], expected_seq, portid)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_commands_sets_mark_hex() {
        let cfg = MarkConfig {
            mark: 0x1,
            exclude_ips: Vec::new(),
        };
        let cmds = build_commands(&cfg, "proxyvpn_mark", "output");
        assert!(cmds.iter().any(|cmd| cmd.contains(&"0x1".to_string())));
    }

    #[test]
    fn build_commands_marks_all_tcp() {
        let cfg = MarkConfig {
            mark: 0x1,
            exclude_ips: Vec::new(),
        };
        let cmds = build_commands(&cfg, "proxyvpn_mark", "output");
        assert!(!cmds.iter().any(|cmd| cmd.contains(&"0x0".to_string())));
    }

    #[test]
    fn build_commands_includes_excludes() {
        let cfg = MarkConfig {
            mark: 0x1,
            exclude_ips: vec!["1.2.3.4".parse().unwrap()],
        };
        let cmds = build_commands(&cfg, "proxyvpn_mark", "output");
        assert!(cmds.iter().any(|cmd| cmd.contains(&"daddr".to_string())));
    }
}
