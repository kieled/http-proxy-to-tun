use smoltcp::wire::{IpProtocol, Ipv4Packet, TcpPacket};

use crate::conn::ConnKey;

pub(crate) fn sniff_syn(packet: &[u8]) -> Option<ConnKey> {
    let ipv4 = Ipv4Packet::new_checked(packet).ok()?;
    if ipv4.next_header() != IpProtocol::Tcp {
        return None;
    }
    let tcp = TcpPacket::new_checked(ipv4.payload()).ok()?;
    if !tcp.syn() || tcp.ack() {
        return None;
    }
    Some(ConnKey {
        src_ip: ipv4.src_addr(),
        src_port: tcp.src_port(),
        dst_ip: ipv4.dst_addr(),
        dst_port: tcp.dst_port(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn build_ipv4_tcp_packet(
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
        src_port: u16,
        dst_port: u16,
        syn: bool,
        ack: bool,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; 40];
        buf[0] = 0x45;
        let total_len = buf.len() as u16;
        buf[2..4].copy_from_slice(&total_len.to_be_bytes());
        buf[8] = 64;
        buf[9] = 6;
        buf[12..16].copy_from_slice(&src_ip.octets());
        buf[16..20].copy_from_slice(&dst_ip.octets());

        buf[20..22].copy_from_slice(&src_port.to_be_bytes());
        buf[22..24].copy_from_slice(&dst_port.to_be_bytes());
        buf[32] = 0x50;
        let mut flags = 0u8;
        if syn {
            flags |= 0x02;
        }
        if ack {
            flags |= 0x10;
        }
        buf[33] = flags;
        buf
    }

    #[test]
    fn sniff_syn_accepts_syn_without_ack() {
        let packet = build_ipv4_tcp_packet(
            Ipv4Addr::new(10, 0, 0, 2),
            Ipv4Addr::new(1, 2, 3, 4),
            12345,
            443,
            true,
            false,
        );
        let key = sniff_syn(&packet).unwrap();
        assert_eq!(key.src_ip, Ipv4Addr::new(10, 0, 0, 2));
        assert_eq!(key.dst_ip, Ipv4Addr::new(1, 2, 3, 4));
        assert_eq!(key.src_port, 12345);
        assert_eq!(key.dst_port, 443);
    }

    #[test]
    fn sniff_syn_rejects_ack() {
        let packet = build_ipv4_tcp_packet(
            Ipv4Addr::new(10, 0, 0, 2),
            Ipv4Addr::new(1, 2, 3, 4),
            12345,
            443,
            true,
            true,
        );
        assert!(sniff_syn(&packet).is_none());
    }
}
