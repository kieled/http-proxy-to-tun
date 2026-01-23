use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use proxyvpn_proxy::{ConnectOptions, ProxyConfig, connect_http_connect_with};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ConnKey {
    pub(crate) src_ip: Ipv4Addr,
    pub(crate) src_port: u16,
    pub(crate) dst_ip: Ipv4Addr,
    pub(crate) dst_port: u16,
}

pub(crate) struct ConnState {
    pub(crate) key: ConnKey,
    pub(crate) to_proxy: mpsc::UnboundedSender<Vec<u8>>,
    pub(crate) from_proxy: mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) pending_to_proxy: VecDeque<Vec<u8>>,
    pub(crate) pending_from_proxy: VecDeque<Vec<u8>>,
    proxy: ProxyConfig,
    proxy_socket_mark: Option<u32>,
    pub(crate) connected: bool,
}

impl ConnState {
    pub(crate) fn new(key: ConnKey, proxy: &ProxyConfig, proxy_socket_mark: Option<u32>) -> Self {
        let (to_proxy, _) = mpsc::unbounded_channel();
        let (_, from_proxy) = mpsc::unbounded_channel();
        Self {
            key,
            to_proxy,
            from_proxy,
            pending_to_proxy: VecDeque::new(),
            pending_from_proxy: VecDeque::new(),
            proxy: proxy.clone(),
            proxy_socket_mark,
            connected: false,
        }
    }

    pub(crate) fn spawn_proxy_task(&mut self) {
        if self.connected {
            return;
        }
        let (to_proxy, mut rx_from_local) = mpsc::unbounded_channel::<Vec<u8>>();
        let (tx_to_local, from_proxy) = mpsc::unbounded_channel::<Vec<u8>>();
        let proxy = self.proxy.clone();
        let dst = SocketAddrV4::new(self.key.dst_ip, self.key.dst_port);
        let options = ConnectOptions {
            socket_mark: self.proxy_socket_mark,
            connect_timeout: None,
        };
        let mut pending = VecDeque::new();
        std::mem::swap(&mut pending, &mut self.pending_to_proxy);
        tokio::spawn(async move {
            if let Ok((mut stream, leftover)) = connect_http_connect_with(&proxy, dst, &options).await {
                if let Some(data) = leftover {
                    let _ = tx_to_local.send(data);
                }
                while let Some(data) = pending.pop_front() {
                    if stream.write_all(&data).await.is_err() {
                        return;
                    }
                }
                let mut buf = vec![0u8; 16 * 1024];
                loop {
                    tokio::select! {
                        Some(data) = rx_from_local.recv() => {
                            if stream.write_all(&data).await.is_err() {
                                break;
                            }
                        }
                        res = stream.read(&mut buf) => {
                            match res {
                                Ok(0) => break,
                                Ok(n) => {
                                    let _ = tx_to_local.send(buf[..n].to_vec());
                                }
                                Err(_) => break,
                            }
                        }
                    }
                }
            }
        });
        self.to_proxy = to_proxy;
        self.from_proxy = from_proxy;
        self.connected = true;
    }
}
