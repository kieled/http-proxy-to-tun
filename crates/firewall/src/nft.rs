use std::ffi::CString;
use std::net::{IpAddr, Ipv4Addr};

use anyhow::{anyhow, Context, Result};
use nix::libc;
use nftnl::{
    Batch, Chain, FinalizedBatch, Hook, MsgType, Policy, ProtoFamily, Rule,
    Table, nft_expr,
};

use proxyvpn_util::CommandRunner;

use crate::FirewallConfig;

pub(crate) fn find_nft_binary() -> bool {
    proxyvpn_util::find_in_path("nft").is_some()
}

pub(crate) fn build_cmds(cfg: &FirewallConfig, table: &str, chain: &str) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    cmds.push(vec!["delete", "table", "inet", table].into_iter().map(String::from).collect());
    cmds.push(vec!["add", "table", "inet", table].into_iter().map(String::from).collect());
    cmds.push(vec![
        "add", "chain", "inet", table, chain, "{", "type", "filter", "hook", "output",
        "priority", "0", ";", "policy", "drop", ";", "}",
    ].into_iter().map(String::from).collect());

    cmds.push(vec![
        "add".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "oifname".to_string(),
        "lo".to_string(),
        "accept".to_string(),
    ]);
    cmds.push(vec![
        "add".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "oifname".to_string(),
        cfg.tun_name.to_string(),
        "accept".to_string(),
    ]);
    cmds.push(vec![
        "add".to_string(),
        "rule".to_string(),
        "inet".to_string(),
        table.to_string(),
        chain.to_string(),
        "meta".to_string(),
        "mark".to_string(),
        format!("0x{:x}", cfg.proxy_mark),
        "accept".to_string(),
    ]);

    for ip in cfg.proxy_ips {
        let port = cfg.proxy_port.to_string();
        cmds.push(vec![
            "add".to_string(),
            "rule".to_string(),
            "inet".to_string(),
            table.to_string(),
            chain.to_string(),
            "ip".to_string(),
            "daddr".to_string(),
            ip.to_string(),
            "tcp".to_string(),
            "dport".to_string(),
            port,
            "accept".to_string(),
        ]);
    }

    for ip in cfg.dns_ips.iter().filter(|ip| ip.is_ipv4()) {
        if cfg.allow_udp_dns {
            cmds.push(vec![
                "add".to_string(),
                "rule".to_string(),
                "inet".to_string(),
                table.to_string(),
                chain.to_string(),
                "ip".to_string(),
                "daddr".to_string(),
                ip.to_string(),
                "udp".to_string(),
                "dport".to_string(),
                "53".to_string(),
                "accept".to_string(),
            ]);
        }
        cmds.push(vec![
            "add".to_string(),
            "rule".to_string(),
            "inet".to_string(),
            table.to_string(),
            chain.to_string(),
            "ip".to_string(),
            "daddr".to_string(),
            ip.to_string(),
            "tcp".to_string(),
            "dport".to_string(),
            "53".to_string(),
            "accept".to_string(),
        ]);
    }

    cmds
}

pub(crate) fn apply_cmd(
    cfg: &FirewallConfig,
    table: &str,
    chain: &str,
    runner: &CommandRunner,
) -> Result<()> {
    let cmds = build_cmds(cfg, table, chain);
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

pub(crate) fn apply_native(cfg: &FirewallConfig, table: &str, chain: &str) -> Result<()> {
    let table_c = CString::new(table).context("invalid nft table name")?;
    let chain_c = CString::new(chain).context("invalid nft chain name")?;
    let table = Table::new(table_c.as_c_str(), ProtoFamily::Inet);
    let mut chain = Chain::new(chain_c.as_c_str(), &table);
    chain.set_hook(Hook::Out, 0);
    chain.set_policy(Policy::Drop);

    let _ = delete_table(table_c.to_str().unwrap_or_default());

    let mut batch = Batch::new();
    batch.add(&table, MsgType::Add);
    batch.add(&chain, MsgType::Add);

    let lo_idx = iface_index("lo")?;
    let tun_idx = iface_index(cfg.tun_name)?;
    add_rule_accept_oif(&mut batch, &chain, lo_idx);
    add_rule_accept_oif(&mut batch, &chain, tun_idx);
    add_rule_accept_mark(&mut batch, &chain, cfg.proxy_mark);

    for ip in cfg.proxy_ips.iter().filter_map(|ip| match ip {
        IpAddr::V4(v4) => Some(*v4),
        IpAddr::V6(_) => None,
    }) {
        add_rule_ip_tcp_accept(&mut batch, &chain, ip, cfg.proxy_port);
    }
    for ip in cfg.dns_ips.iter().filter_map(|ip| match ip {
        IpAddr::V4(v4) => Some(*v4),
        IpAddr::V6(_) => None,
    }) {
        if cfg.allow_udp_dns {
            add_rule_ip_udp_accept(&mut batch, &chain, ip, 53);
        }
        add_rule_ip_tcp_accept(&mut batch, &chain, ip, 53);
    }

    let finalized = batch.finalize();
    send_and_process(&finalized)?;
    Ok(())
}

pub(crate) fn delete_table(table: &str) -> Result<()> {
    let table_c = CString::new(table).context("invalid nft table name")?;
    let table = Table::new(table_c.as_c_str(), ProtoFamily::Inet);
    let mut batch = Batch::new();
    batch.add(&table, MsgType::Del);
    let finalized = batch.finalize();
    let _ = send_and_process(&finalized);
    Ok(())
}

fn iface_index(name: &str) -> Result<u32> {
    let cstr = CString::new(name).context("invalid interface name")?;
    let idx = unsafe { libc::if_nametoindex(cstr.as_ptr()) };
    if idx == 0 {
        return Err(anyhow!("interface not found: {name}"));
    }
    Ok(idx)
}

fn add_rule_accept_oif(batch: &mut Batch, chain: &Chain, index: u32) {
    let mut rule = Rule::new(chain);
    rule.add_expr(&nft_expr!(meta oif));
    rule.add_expr(&nft_expr!(cmp == index));
    rule.add_expr(&nft_expr!(verdict accept));
    batch.add(&rule, MsgType::Add);
}

fn add_rule_accept_mark(batch: &mut Batch, chain: &Chain, mark: u32) {
    let mut rule = Rule::new(chain);
    rule.add_expr(&nft_expr!(meta mark));
    rule.add_expr(&nft_expr!(cmp == mark));
    rule.add_expr(&nft_expr!(verdict accept));
    batch.add(&rule, MsgType::Add);
}

fn add_rule_ip_tcp_accept(batch: &mut Batch, chain: &Chain, ip: Ipv4Addr, port: u16) {
    let port = port.to_be();
    let mut rule = Rule::new(chain);
    rule.add_expr(&nft_expr!(meta nfproto));
    rule.add_expr(&nft_expr!(cmp == libc::NFPROTO_IPV4 as u8));
    rule.add_expr(&nft_expr!(payload ipv4 daddr));
    rule.add_expr(&nft_expr!(cmp == ip));
    rule.add_expr(&nft_expr!(meta l4proto));
    rule.add_expr(&nft_expr!(cmp == libc::IPPROTO_TCP as u8));
    rule.add_expr(&nft_expr!(payload tcp dport));
    rule.add_expr(&nft_expr!(cmp == port));
    rule.add_expr(&nft_expr!(verdict accept));
    batch.add(&rule, MsgType::Add);
}

fn add_rule_ip_udp_accept(batch: &mut Batch, chain: &Chain, ip: Ipv4Addr, port: u16) {
    let port = port.to_be();
    let mut rule = Rule::new(chain);
    rule.add_expr(&nft_expr!(meta nfproto));
    rule.add_expr(&nft_expr!(cmp == libc::NFPROTO_IPV4 as u8));
    rule.add_expr(&nft_expr!(payload ipv4 daddr));
    rule.add_expr(&nft_expr!(cmp == ip));
    rule.add_expr(&nft_expr!(meta l4proto));
    rule.add_expr(&nft_expr!(cmp == libc::IPPROTO_UDP as u8));
    rule.add_expr(&nft_expr!(payload udp dport));
    rule.add_expr(&nft_expr!(cmp == port));
    rule.add_expr(&nft_expr!(verdict accept));
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
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn build_cmds_includes_proxy_and_dns_rules() {
        let cfg = FirewallConfig {
            tun_name: "tun0",
            proxy_ips: &[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))],
            proxy_port: 8080,
            dns_ips: &[IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
            allow_udp_dns: true,
            proxy_mark: 0x1,
        };
        let cmds = build_cmds(&cfg, "proxyvpn", "output");
        assert!(cmds.iter().any(|cmd| cmd.contains(&"daddr".to_string()) && cmd.contains(&"203.0.113.1".to_string())));
        assert!(cmds.iter().any(|cmd| cmd.contains(&"udp".to_string()) && cmd.contains(&"53".to_string())));
        assert!(cmds.iter().any(|cmd| cmd.contains(&"mark".to_string()) && cmd.contains(&"0x1".to_string())));
    }
}
