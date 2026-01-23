use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use clap::Parser;
use proxyvpn_util::dns;
use proxyvpn_proxy::ProxyConfig;

mod proxy_dns;

#[derive(Parser, Debug)]
#[command(about = "ProxyVPN DNS/routing self-test")]
struct Args {
    #[arg(long, default_value = "ifconfig.me")]
    name: String,
    #[arg(long)]
    server: Option<Ipv4Addr>,
    #[arg(long, default_value_t = 1500)]
    timeout_ms: u64,
    #[arg(long)]
    no_ip: bool,

    /// Proxy URL for DNS-over-TCP via HTTP CONNECT (http://user:pass@host:port)
    #[arg(long)]
    proxy_url: Option<String>,

    /// Set SO_MARK on proxy TCP socket (Linux only)
    #[arg(long)]
    socket_mark: Option<u32>,

    /// Exit with non-zero status on probe failure
    #[arg(long)]
    strict: bool,
}

#[derive(Debug)]
struct DnsProbeResult {
    id: u16,
    rcode: u16,
    ancount: u16,
    duration: Duration,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {:#}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let server = match args.server {
        Some(ip) => ip,
        None => dns::first_resolv_conf_v4("/etc/resolv.conf")
            .ok_or_else(|| anyhow!("no IPv4 nameserver found in /etc/resolv.conf"))?,
    };

    if let Some(proxy_url) = &args.proxy_url {
        let proxy = ProxyConfig::from_http_url(proxy_url)?;
        let options = proxyvpn_proxy::ConnectOptions {
            socket_mark: args.socket_mark,
            connect_timeout: None,
        };
        match proxy_dns::dns_probe_tcp_via_proxy_with_options(
            proxy,
            server,
            &args.name,
            Duration::from_millis(args.timeout_ms),
            options,
        ) {
            Ok(result) => {
                println!(
                    "dns_probe_tcp_via_proxy {} {}: OK id={} rcode={} an={} time_ms={:.1}",
                    server,
                    args.name,
                    result.id,
                    result.rcode,
                    result.ancount,
                    result.duration.as_secs_f64() * 1000.0
                );
            }
            Err(err) => {
                println!(
                    "dns_probe_tcp_via_proxy {} {}: FAIL error: {:#}",
                    server, args.name, err
                );
                if args.strict {
                    return Err(err);
                }
            }
        }
    } else {
        match dns_probe(server, &args.name, Duration::from_millis(args.timeout_ms)) {
            Ok(result) => {
                println!(
                    "dns_probe {} {}: OK id={} rcode={} an={} time_ms={:.1}",
                    server,
                    args.name,
                    result.id,
                    result.rcode,
                    result.ancount,
                    result.duration.as_secs_f64() * 1000.0
                );
            }
            Err(err) => {
                println!("dns_probe {} {}: FAIL error: {}", server, args.name, err);
                if args.strict {
                    return Err(err);
                }
            }
        }
    }

    if !args.no_ip {
        print_ip("-4", &["rule", "show"]);
        for target in [
            IpAddr::V4(server),
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
        ] {
            print_ip("-4", &["route", "get", &target.to_string()]);
        }
    }

    Ok(())
}

fn dns_probe(server: Ipv4Addr, name: &str, timeout: Duration) -> Result<DnsProbeResult> {
    if name.trim().is_empty() {
        return Err(anyhow!("empty name"));
    }

    let id = make_id();
    let query = build_query(name, id)?;
    let sock = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
    sock.set_read_timeout(Some(timeout))?;

    let start = Instant::now();
    sock.send_to(&query, SocketAddr::from((server, 53)))?;
    let mut buf = [0u8; 512];
    let (size, _from) = sock.recv_from(&mut buf)?;
    let duration = start.elapsed();

    parse_response(id, &buf[..size]).map(|(rcode, ancount)| DnsProbeResult {
        id,
        rcode,
        ancount,
        duration,
    })
}

fn build_query(name: &str, id: u16) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&0x0100u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());

    for label in name.split('.') {
        if label.is_empty() {
            return Err(anyhow!("invalid DNS name"));
        }
        if label.len() > 63 {
            return Err(anyhow!("DNS label too long"));
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    Ok(out)
}

fn parse_response(expected_id: u16, data: &[u8]) -> Result<(u16, u16)> {
    if data.len() < 12 {
        return Err(anyhow!("short DNS response"));
    }
    let id = u16::from_be_bytes([data[0], data[1]]);
    if id != expected_id {
        return Err(anyhow!("DNS response id mismatch"));
    }
    let flags = u16::from_be_bytes([data[2], data[3]]);
    let rcode = flags & 0x000f;
    let ancount = u16::from_be_bytes([data[6], data[7]]);
    Ok((rcode, ancount))
}

fn print_ip(prefix: &str, args: &[&str]) {
    let mut cmd_args = Vec::with_capacity(args.len() + 1);
    cmd_args.push(prefix);
    cmd_args.extend_from_slice(args);

    println!("## ip {}", cmd_args.join(" "));
    match Command::new("ip").args(&cmd_args).output() {
        Ok(output) => {
            if !output.stdout.is_empty() {
                print_string("", &output.stdout);
            }
            if !output.stderr.is_empty() {
                print_string("", &output.stderr);
            }
            if !output.status.success() {
                println!("(command failed: {})", output.status.code().unwrap_or(1));
            }
        }
        Err(err) => println!("(command failed: {})", err),
    }
}

fn print_string(prefix: &str, data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        for line in text.lines() {
            println!("{}{}", prefix, line);
        }
    }
}

fn make_id() -> u16 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.subsec_nanos() as u16;
    let pid = std::process::id() as u16;
    nanos ^ pid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_rejects_empty_label() {
        assert!(build_query("example..com", 1).is_err());
    }

    #[test]
    fn build_query_contains_header() {
        let query = build_query("example.com", 0x1234).unwrap();
        assert_eq!(&query[0..2], &0x1234u16.to_be_bytes());
        assert_eq!(&query[2..4], &0x0100u16.to_be_bytes());
    }

    #[test]
    fn parse_response_rejects_short() {
        let err = parse_response(1, &[0u8; 2]).unwrap_err();
        assert!(err.to_string().contains("short"));
    }
}
