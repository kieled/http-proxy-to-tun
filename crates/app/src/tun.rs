use std::net::Ipv4Addr;

use anyhow::{Context, Result, anyhow};

use super::ops::NetlinkOps;

pub(crate) async fn ensure_tun_cidr_free<N: NetlinkOps>(
    netlink: &N,
    tun_ip: Ipv4Addr,
    prefix: u8,
) -> Result<()> {
    let addrs = netlink.ipv4_addrs().await?;
    if let Some(overlap) = find_overlapping_addr(&addrs, tun_ip, prefix) {
        return Err(anyhow!(
            "TUN CIDR {}/{} overlaps with existing address {}; choose a different --tun-cidr",
            tun_ip,
            prefix,
            overlap
        ));
    }
    Ok(())
}

pub(crate) fn find_overlapping_addr(
    addrs: &[Ipv4Addr],
    tun_ip: Ipv4Addr,
    prefix: u8,
) -> Option<Ipv4Addr> {
    let mask = if prefix == 0 {
        0
    } else {
        (!0u32).checked_shl(32 - prefix as u32).unwrap_or(0)
    };
    let tun_net = u32::from(tun_ip) & mask;
    for addr in addrs {
        let addr_u = u32::from(*addr);
        if (addr_u & mask) == tun_net {
            return Some(*addr);
        }
    }
    None
}

pub(crate) fn create_tun_device(
    tun_name: &str,
    tun_ip: Ipv4Addr,
    tun_netmask: Ipv4Addr,
) -> Result<tun::AsyncDevice> {
    let mut cfg = tun::Configuration::default();
    cfg.name(tun_name)
        .address(tun_ip)
        .netmask(tun_netmask)
        .up();
    cfg.platform(|platform| {
        platform.packet_information(false);
    });
    tun::create_as_async(&cfg).context("failed to open TUN device")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_overlapping_addr_detects_overlap() {
        let addrs = [
            Ipv4Addr::new(192, 168, 0, 103),
            Ipv4Addr::new(172, 19, 0, 1),
        ];
        let overlap = find_overlapping_addr(&addrs, Ipv4Addr::new(172, 19, 0, 1), 30);
        assert_eq!(overlap, Some(Ipv4Addr::new(172, 19, 0, 1)));
    }

    #[test]
    fn find_overlapping_addr_none_for_other_subnet() {
        let addrs = [Ipv4Addr::new(192, 168, 0, 103)];
        let overlap = find_overlapping_addr(&addrs, Ipv4Addr::new(10, 255, 255, 1), 30);
        assert!(overlap.is_none());
    }
}
