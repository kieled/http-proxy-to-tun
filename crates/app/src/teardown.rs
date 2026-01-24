use anyhow::Result;

use proxyvpn_state::State;

use super::ops::{FirewallOps, MarkOps, NetlinkOps, StateStoreOps};

pub async fn teardown<N, F, M, S>(
    state: &State,
    store: &S,
    netlink: &N,
    firewall: &F,
    mark: &M,
    keep_logs: bool,
) -> Result<()>
where
    N: NetlinkOps,
    F: FirewallOps,
    M: MarkOps,
    S: StateStoreOps,
{
    // Remove firewall rules first (critical for connectivity)
    if let Some(fw) = &state.firewall
        && let Err(e) = firewall.remove_from_state(fw)
    {
        eprintln!("teardown: failed to remove firewall: {e}");
    }

    // Remove mark rules (critical - these break networking if left behind)
    if let Err(e) = mark.remove_best_effort() {
        eprintln!("teardown: failed to remove mark rules: {e}");
    }

    // Remove routing rules
    if let Some(pref) = state.tcp_rule_pref
        && let Err(e) = netlink.delete_rule_pref(pref).await
    {
        eprintln!("teardown: failed to delete tcp rule {pref}: {e}");
    }
    for rule in &state.dns_bypass_rules {
        if let Err(e) = netlink.delete_rule_pref(rule.pref).await {
            eprintln!("teardown: failed to delete dns rule {}: {e}", rule.pref);
        }
    }
    for rule in &state.proxy_bypass_rules {
        if let Err(e) = netlink.delete_rule_pref(rule.pref).await {
            eprintln!("teardown: failed to delete proxy rule {}: {e}", rule.pref);
        }
    }

    // Remove routes in proxy table
    if let Err(e) = netlink.delete_routes_in_table(state.proxy_table).await {
        eprintln!(
            "teardown: failed to delete routes in table {}: {e}",
            state.proxy_table
        );
    }

    store.remove_state_files(keep_logs)?;
    Ok(())
}
