use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use smoltcp::iface::{Config as IfaceConfig, Interface, SocketHandle, SocketSet};
use smoltcp::socket::tcp::{Socket as TcpSocket, SocketBuffer as TcpSocketBuffer, State as TcpState};
use smoltcp::time::Instant as SmolInstant;
use smoltcp::wire::{IpAddress, IpCidr};
use tun::Device as TunDeviceTrait;

use proxyvpn_proxy::ProxyConfig;

use crate::conn::{ConnKey, ConnState};
use crate::device::QueueDevice;
use crate::packet::sniff_syn;

pub struct TunStackConfig {
    pub tun_ip: Ipv4Addr,
    pub tun_prefix: u8,
    pub proxy: ProxyConfig,
    pub proxy_socket_mark: Option<u32>,
}

pub async fn run_tun_stack(
    tun_dev: tun::AsyncDevice,
    cfg: TunStackConfig,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let mtu = tun_dev.get_ref().mtu().unwrap_or(1500) as usize;
    let mut framed = tun_dev.into_framed();
    let mut device = QueueDevice::new(mtu);

    let mut iface = build_interface(&mut device, cfg.tun_ip, cfg.tun_prefix)?;
    let mut sockets = SocketSet::new(vec![]);
    let mut by_key: HashMap<ConnKey, SocketHandle> = HashMap::new();
    let mut conns: HashMap<SocketHandle, ConnState> = HashMap::new();

    let start = std::time::Instant::now();
    loop {
        let now = smol_now(start);
        let poll_delay = iface
            .poll_delay(now, &sockets)
            .map(|delay| Duration::from_millis(delay.total_millis()))
            .unwrap_or(Duration::from_millis(10));

        tokio::select! {
            _ = &mut shutdown => {
                break;
            }
            maybe_pkt = framed.next() => {
                match maybe_pkt {
                    Some(Ok(pkt)) => {
                        let bytes = pkt.into_bytes().to_vec();
                        if let Some(key) = sniff_syn(&bytes)
                            && let std::collections::hash_map::Entry::Vacant(e) = by_key.entry(key)
                        {
                            let handle = create_socket(&mut sockets, key.dst_ip, key.dst_port)?;
                            e.insert(handle);
                            conns.insert(handle, ConnState::new(key, &cfg.proxy, cfg.proxy_socket_mark));
                        }
                        device.push_rx(bytes);
                    }
                    Some(Err(err)) => return Err(err.into()),
                    None => break,
                }
            }
            _ = tokio::time::sleep(poll_delay) => {}
        }

        let now = smol_now(start);
        let _ = iface.poll(now, &mut device, &mut sockets);

        let handles: Vec<SocketHandle> = sockets.iter_mut().map(|(h, _)| h).collect();
        for handle in handles {
            let mut remove = false;
            let socket = sockets.get_mut::<TcpSocket>(handle);
            if let Some(conn) = conns.get_mut(&handle) {
                if !conn.connected && socket.state() == TcpState::Established {
                    conn.spawn_proxy_task();
                }

                while let Ok(data) = conn.from_proxy.try_recv() {
                    conn.pending_from_proxy.push_back(data);
                }

                if socket.can_send() {
                    while let Some(mut data) = conn.pending_from_proxy.pop_front() {
                        let sent = socket.send_slice(&data)?;
                        if sent < data.len() {
                            data.drain(0..sent);
                            conn.pending_from_proxy.push_front(data);
                            break;
                        }
                    }
                }

                if socket.can_recv() {
                    let _ = socket.recv(|buf| {
                        if !buf.is_empty() {
                            if conn.connected {
                                let _ = conn.to_proxy.send(buf.to_vec());
                            } else {
                                conn.pending_to_proxy.push_back(buf.to_vec());
                            }
                        }
                        (buf.len(), ())
                    });
                }

                if socket.state() == TcpState::Closed || socket.state() == TcpState::TimeWait {
                    remove = true;
                }

                if conn.connected && conn.from_proxy.is_closed() && conn.pending_from_proxy.is_empty() {
                    socket.abort();
                    remove = true;
                }
            }

            if remove {
                conns.remove(&handle);
                by_key.retain(|_, v| *v != handle);
                sockets.remove(handle);
            }
        }

        while let Some(pkt) = device.pop_tx() {
            framed.send(tun::r#async::TunPacket::new(pkt)).await?;
        }
    }
    Ok(())
}

fn build_interface(
    device: &mut QueueDevice,
    tun_ip: Ipv4Addr,
    tun_prefix: u8,
) -> Result<Interface> {
    let mut config = IfaceConfig::new(smoltcp::wire::HardwareAddress::Ip);
    config.random_seed = rand_seed();
    let mut iface = Interface::new(config, device, SmolInstant::from_millis(0));
    iface.update_ip_addrs(|addrs| {
        let cidr = IpCidr::new(IpAddress::Ipv4(tun_ip), tun_prefix);
        let _ = addrs.push(cidr);
    });
    iface.set_any_ip(true);
    iface
        .routes_mut()
        .add_default_ipv4_route(tun_ip)
        .context("failed to add default route")?;
    Ok(iface)
}

fn rand_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn smol_now(start: std::time::Instant) -> SmolInstant {
    let elapsed = start.elapsed();
    SmolInstant::from_millis(elapsed.as_millis() as i64)
}

fn create_socket(
    sockets: &mut SocketSet<'_>,
    dst_ip: Ipv4Addr,
    dst_port: u16,
) -> Result<SocketHandle> {
    let rx = TcpSocketBuffer::new(vec![0; 64 * 1024]);
    let tx = TcpSocketBuffer::new(vec![0; 64 * 1024]);
    let mut socket = TcpSocket::new(rx, tx);
    socket
        .listen(SocketAddrV4::new(dst_ip, dst_port))
        .map_err(|_| anyhow!("failed to listen on {}:{}", dst_ip, dst_port))?;
    Ok(sockets.add(socket))
}
