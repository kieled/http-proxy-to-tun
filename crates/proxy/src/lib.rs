use std::net::{SocketAddr, SocketAddrV4};
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use url::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpSocket, TcpStream};

#[derive(Clone, Debug)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl ProxyConfig {
    pub fn from_http_url(raw: &str) -> Result<Self> {
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
        Ok(Self {
            host,
            port,
            username: username.to_string(),
            password: password.to_string(),
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConnectOptions {
    pub socket_mark: Option<u32>,
    pub connect_timeout: Option<Duration>,
}

pub async fn connect_http_connect(
    proxy: &ProxyConfig,
    target: SocketAddrV4,
) -> Result<(TcpStream, Option<Vec<u8>>)> {
    connect_http_connect_with(proxy, target, &ConnectOptions::default()).await
}

pub async fn connect_http_connect_with(
    proxy: &ProxyConfig,
    target: SocketAddrV4,
    options: &ConnectOptions,
) -> Result<(TcpStream, Option<Vec<u8>>)> {
    let mut stream = connect_proxy_stream(proxy, options).await?;

    let auth = base64::engine::general_purpose::STANDARD.encode(format!(
        "{}:{}",
        proxy.username, proxy.password
    ));
    let connect_req = format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\nProxy-Authorization: Basic {}\r\n\r\n",
        target.ip(),
        target.port(),
        target.ip(),
        target.port(),
        auth
    );
    stream.write_all(connect_req.as_bytes()).await?;

    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 512];
    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(anyhow!("proxy closed during CONNECT"));
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 16 * 1024 {
            return Err(anyhow!("proxy CONNECT response too large"));
        }
    }

    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut res = httparse::Response::new(&mut headers);
    match res.parse(&buf)? {
        httparse::Status::Complete(n) => {
            let code = res.code.unwrap_or(0);
            if code != 200 {
                return Err(anyhow!("proxy CONNECT failed: HTTP {code}"));
            }
            let leftover = if n < buf.len() {
                Some(buf[n..].to_vec())
            } else {
                None
            };
            Ok((stream, leftover))
        }
        httparse::Status::Partial => Err(anyhow!("proxy CONNECT response incomplete")),
    }
}

async fn connect_proxy_stream(proxy: &ProxyConfig, options: &ConnectOptions) -> Result<TcpStream> {
    let addr = resolve_proxy_addr(proxy).await?;
    let socket = match addr {
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
    };

    if let Some(mark) = options.socket_mark {
        set_socket_mark(&socket, mark)?;
    }

    let connect_fut = socket.connect(addr);
    let stream = if let Some(timeout) = options.connect_timeout {
        match tokio::time::timeout(timeout, connect_fut).await {
            Ok(res) => res.with_context(|| {
                format!("failed to connect to proxy {}:{}", proxy.host, proxy.port)
            })?,
            Err(_) => return Err(anyhow!("proxy connect timeout")),
        }
    } else {
        connect_fut
            .await
            .with_context(|| format!("failed to connect to proxy {}:{}", proxy.host, proxy.port))?
    };
    Ok(stream)
}

async fn resolve_proxy_addr(proxy: &ProxyConfig) -> Result<SocketAddr> {
    let addrs = tokio::net::lookup_host((proxy.host.as_str(), proxy.port))
        .await
        .with_context(|| format!("failed to resolve proxy host {}", proxy.host))?;
    let mut first = None;
    let mut first_v4 = None;
    for addr in addrs {
        if first.is_none() {
            first = Some(addr);
        }
        if matches!(addr, SocketAddr::V4(_)) {
            first_v4 = Some(addr);
            break;
        }
    }
    first_v4
        .or(first)
        .ok_or_else(|| anyhow!("proxy host did not resolve to any IPs"))
}

#[cfg(target_os = "linux")]
fn set_socket_mark(socket: &TcpSocket, mark: u32) -> Result<()> {
    let fd = socket.as_raw_fd();
    let value: libc::c_uint = mark;
    let ret = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_MARK,
            &value as *const _ as *const libc::c_void,
            std::mem::size_of_val(&value) as libc::socklen_t,
        )
    };
    if ret != 0 {
        return Err(anyhow!(std::io::Error::last_os_error())
            .context("failed to set SO_MARK on proxy socket"));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn set_socket_mark(_socket: &TcpSocket, _mark: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn connect_http_connect_success_with_leftover() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let proxy = ProxyConfig {
            host: "127.0.0.1".to_string(),
            port: addr.port(),
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let target = SocketAddrV4::new(Ipv4Addr::new(1, 2, 3, 4), 443);

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("CONNECT 1.2.3.4:443"));
            assert!(req.contains("Proxy-Authorization: Basic"));
            let response = b"HTTP/1.1 200 Connection Established\r\n\r\nleftover";
            socket.write_all(response).await.unwrap();
        });

        let (_stream, leftover) = connect_http_connect(&proxy, target).await.unwrap();
        assert_eq!(leftover, Some(b"leftover".to_vec()));
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn connect_http_connect_rejects_non_200() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let proxy = ProxyConfig {
            host: "127.0.0.1".to_string(),
            port: addr.port(),
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let target = SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 80);

        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let _ = socket.read(&mut buf).await.unwrap();
            let response = b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n";
            socket.write_all(response).await.unwrap();
        });

        let err = connect_http_connect(&proxy, target).await.unwrap_err();
        assert!(err.to_string().contains("HTTP 407"));
        handle.await.unwrap();
    }
}
