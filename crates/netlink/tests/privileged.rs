#![cfg(feature = "privileged-tests")]

use proxyvpn_netlink::Netlink;

#[tokio::test]
#[ignore]
async fn can_list_ipv4_addrs() {
    if std::env::var("PROXYVPN_PRIV_TESTS_ALLOW_NETLINK").ok().as_deref() != Some("1") {
        eprintln!("skipping netlink test (set PROXYVPN_PRIV_TESTS_ALLOW_NETLINK=1)");
        return;
    }

    let netlink = Netlink::new().unwrap();
    let addrs = netlink.ipv4_addrs().await.unwrap();
    if addrs.is_empty() {
        if std::env::var("PROXYVPN_PRIV_TESTS_REQUIRE_ADDRS").ok().as_deref() == Some("1") {
            panic!("no IPv4 addresses found; require addrs requested");
        }
        eprintln!("no IPv4 addresses found (likely netns without loopback); skipping assert");
        return;
    }
}
