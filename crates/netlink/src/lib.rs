use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};

use anyhow::{anyhow, Context, Result};
use futures_util::TryStreamExt;
use netlink_packet_route::address::AddressAttribute;
use netlink_packet_route::route::{RouteAttribute, RouteMessage};
use netlink_packet_route::rule::{RuleAction, RuleAttribute};
use rtnetlink::{new_connection, Handle, IpVersion, RouteMessageBuilder};

pub struct Netlink {
    handle: Handle,
    _task: tokio::task::JoinHandle<()>,
}

impl Netlink {
    pub fn new() -> Result<Self> {
        let (conn, handle, _) = new_connection().context("failed to open netlink connection")?;
        let task = tokio::spawn(conn);
        Ok(Self { handle, _task: task })
    }

    pub async fn link_index(&self, name: &str) -> Result<u32> {
        let mut links = self
            .handle
            .link()
            .get()
            .match_name(name.to_string())
            .execute();
        if let Some(msg) = links.try_next().await? {
            return Ok(msg.header.index);
        }
        Err(anyhow!("interface not found: {name}"))
    }

    pub async fn ipv4_addrs(&self) -> Result<Vec<Ipv4Addr>> {
        let mut addrs = Vec::new();
        let mut req = self.handle.address().get().execute();
        while let Some(addr) = req.try_next().await? {
            for attr in &addr.attributes {
                match attr {
                    AddressAttribute::Address(IpAddr::V4(v4))
                    | AddressAttribute::Local(IpAddr::V4(v4)) => {
                        addrs.push(*v4);
                    }
                    _ => {}
                }
            }
        }
        addrs.sort();
        addrs.dedup();
        Ok(addrs)
    }

    pub async fn add_default_route_to_table(
        &self,
        tun_name: &str,
        tun_ip: Ipv4Addr,
        table: u32,
    ) -> Result<()> {
        let idx = self.link_index(tun_name).await?;

        // Build route message for default route (0.0.0.0/0)
        let route = RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(Ipv4Addr::UNSPECIFIED, 0)
            .output_interface(idx)
            .pref_source(tun_ip)
            .table_id(table)
            .build();

        self.handle
            .route()
            .add(route)
            .replace()
            .execute()
            .await?;
        Ok(())
    }

    pub async fn delete_routes_in_table(&self, table: u32) -> Result<()> {
        // Get IPv4 routes
        let filter = RouteMessageBuilder::<Ipv4Addr>::new().build();
        let mut req = self.handle.route().get(filter).execute();
        let mut to_delete = Vec::new();
        while let Some(route) = req.try_next().await? {
            if route_table_id(&route) != table {
                continue;
            }
            to_delete.push(route);
        }
        for route in to_delete {
            let _ = self.handle.route().del(route).execute().await;
        }
        Ok(())
    }

    pub async fn add_rule_fwmark_table(&self, pref: u32, table: u32, mark: u32) -> Result<()> {
        let mask = 0x1;
        let mut req = self
            .handle
            .rule()
            .add()
            .v4()
            .action(RuleAction::ToTable)
            .table_id(table)
            .priority(pref);
        req.message_mut()
            .attributes
            .push(RuleAttribute::FwMark(mark));
        req.message_mut()
            .attributes
            .push(RuleAttribute::FwMask(mask));
        req.execute().await.context("failed to add fwmark rule")?;
        Ok(())
    }

    pub async fn add_rule_to_ip(&self, pref: u32, ip: Ipv4Addr, table: u32) -> Result<()> {
        self.handle
            .rule()
            .add()
            .v4()
            .action(RuleAction::ToTable)
            .table_id(table)
            .priority(pref)
            .destination_prefix(ip, 32)
            .execute()
            .await
            .context("failed to add destination rule")?;
        Ok(())
    }

    pub async fn delete_rule_pref(&self, pref: u32) -> Result<()> {
        let mut req = self.handle.rule().get(IpVersion::V4).execute();
        let mut to_delete = Vec::new();
        while let Some(rule) = req.try_next().await? {
            let mut priority = None;
            for attr in &rule.attributes {
                if let RuleAttribute::Priority(p) = attr {
                    priority = Some(*p);
                    break;
                }
            }
            if priority == Some(pref) {
                to_delete.push(rule);
            }
        }
        for rule in to_delete {
            let _ = self.handle.rule().del(rule).execute().await;
        }
        Ok(())
    }

    pub async fn existing_rule_prefs(&self) -> Result<HashSet<u32>> {
        let mut prefs = HashSet::new();
        let mut req = self.handle.rule().get(IpVersion::V4).execute();
        while let Some(rule) = req.try_next().await? {
            for attr in &rule.attributes {
                if let RuleAttribute::Priority(p) = attr {
                    prefs.insert(*p);
                }
            }
        }
        Ok(prefs)
    }
}

fn route_table_id(route: &RouteMessage) -> u32 {
    for attr in &route.attributes {
        if let RouteAttribute::Table(value) = attr {
            return *value;
        }
    }
    route.header.table as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_table_id_prefers_attr() {
        let mut msg = RouteMessage::default();
        msg.header.table = 5;
        msg.attributes.push(RouteAttribute::Table(254));
        assert_eq!(route_table_id(&msg), 254);
    }

    #[test]
    fn route_table_id_falls_back_to_header() {
        let mut msg = RouteMessage::default();
        msg.header.table = 100;
        assert_eq!(route_table_id(&msg), 100);
    }
}
