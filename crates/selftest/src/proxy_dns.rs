use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use anyhow::{Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use proxyvpn_proxy::{ConnectOptions, ProxyConfig, connect_http_connect_with};

use crate::{DnsProbeResult, build_query, parse_response, make_id};

#[allow(dead_code)]
pub fn dns_probe_tcp_via_proxy(
    proxy: ProxyConfig,
    server: Ipv4Addr,
    name: &str,
    timeout: Duration,
) -> Result<DnsProbeResult> {
    dns_probe_tcp_via_proxy_with_options(proxy, server, name, timeout, ConnectOptions::default())
}

pub fn dns_probe_tcp_via_proxy_with_options(
    proxy: ProxyConfig,
    server: Ipv4Addr,
    name: &str,
    timeout: Duration,
    options: ConnectOptions,
) -> Result<DnsProbeResult> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(dns_probe_tcp_via_proxy_async(proxy, server, name, timeout, options))
}

async fn dns_probe_tcp_via_proxy_async(
    proxy: ProxyConfig,
    server: Ipv4Addr,
    name: &str,
    timeout: Duration,
    options: ConnectOptions,
) -> Result<DnsProbeResult> {
    if name.trim().is_empty() {
        return Err(anyhow!("empty name"));
    }

    let id = make_id();
    let query = build_query(name, id)?;
    let query_len = (query.len() as u16).to_be_bytes();

    let start = std::time::Instant::now();
    let target = SocketAddrV4::new(server, 53);

    let (mut stream, leftover) = tokio::time::timeout(
        timeout,
        connect_http_connect_with(&proxy, target, &options),
    )
    .await
    .map_err(|_| anyhow!("proxy CONNECT timeout"))??;
    let mut leftover = leftover.unwrap_or_default();

    tokio::time::timeout(timeout, async {
        stream.write_all(&query_len).await?;
        stream.write_all(&query).await?;
        Ok::<(), anyhow::Error>(())
    })
    .await
    .map_err(|_| anyhow!("dns query write timeout"))??;

    let len_bytes = read_exact_with_leftover(&mut stream, &mut leftover, 2, timeout).await?;
    let resp_len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
    let resp = read_exact_with_leftover(&mut stream, &mut leftover, resp_len, timeout).await?;
    let duration = start.elapsed();

    parse_response(id, &resp).map(|(rcode, ancount)| DnsProbeResult {
        id,
        rcode,
        ancount,
        duration,
    })
}

async fn read_exact_with_leftover(
    stream: &mut tokio::net::TcpStream,
    leftover: &mut Vec<u8>,
    len: usize,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        if !leftover.is_empty() {
            let take = (len - out.len()).min(leftover.len());
            out.extend_from_slice(&leftover[..take]);
            leftover.drain(..take);
            continue;
        }
        let mut buf = vec![0u8; len - out.len()];
        let n = tokio::time::timeout(timeout, stream.read(&mut buf))
            .await
            .map_err(|_| anyhow!("dns response read timeout"))??;
        if n == 0 {
            return Err(anyhow!("proxy closed during DNS response"));
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn dns_probe_tcp_via_proxy_ok() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let mut read = 0usize;
            loop {
                let n = socket.read(&mut buf[read..]).await.unwrap();
                if n == 0 {
                    return;
                }
                read += n;
                if buf[..read].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await.unwrap();

            let mut len_buf = [0u8; 2];
            socket.read_exact(&mut len_buf).await.unwrap();
            let qlen = u16::from_be_bytes(len_buf) as usize;
            let mut query = vec![0u8; qlen];
            socket.read_exact(&mut query).await.unwrap();
            let id = u16::from_be_bytes([query[0], query[1]]);

            let mut resp = vec![0u8; 12];
            resp[0..2].copy_from_slice(&id.to_be_bytes());
            resp[2..4].copy_from_slice(&0x8180u16.to_be_bytes());
            resp[6..8].copy_from_slice(&0u16.to_be_bytes());
            let resp_len = (resp.len() as u16).to_be_bytes();
            socket.write_all(&resp_len).await.unwrap();
            socket.write_all(&resp).await.unwrap();
        });

        let proxy = ProxyConfig {
            host: "127.0.0.1".to_string(),
            port: addr.port(),
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let result = dns_probe_tcp_via_proxy_async(
            proxy,
            Ipv4Addr::new(1, 1, 1, 1),
            "example.com",
            Duration::from_secs(1),
            ConnectOptions::default(),
        )
        .await
        .unwrap();

        assert_eq!(result.rcode, 0);
        handle.await.unwrap();
    }
}
