#![cfg(feature = "privileged-tests")]

use proxyvpn_firewall::{FirewallBackend, FirewallConfig, IptablesBackend, NftBackend};
use proxyvpn_util::CommandRunner;

fn allow_firewall_tests() -> bool {
    std::env::var("PROXYVPN_PRIV_TESTS_ALLOW_FIREWALL").ok().as_deref() == Some("1")
}

#[test]
#[ignore]
fn apply_and_remove_firewall_rules() {
    if !allow_firewall_tests() {
        eprintln!("skipping firewall test (set PROXYVPN_PRIV_TESTS_ALLOW_FIREWALL=1)");
        return;
    }

    let runner = CommandRunner::new(true, false);
    let cfg = FirewallConfig {
        tun_name: "lo",
        proxy_ips: &[],
        proxy_port: 8080,
        dns_ips: &[],
        allow_udp_dns: false,
        proxy_mark: 0x1,
    };

    if proxyvpn_util::find_in_path("nft").is_some() {
        let backend = NftBackend {
            table: "proxyvpn_test".to_string(),
            chain: "output".to_string(),
        };
        let state = backend.apply(&cfg, &runner).unwrap();
        backend.remove(&state, &runner).unwrap();
    } else if proxyvpn_util::find_in_path("iptables").is_some() {
        let backend = IptablesBackend {
            chain: "PROXYVPN_TEST".to_string(),
        };
        let state = backend.apply(&cfg, &runner).unwrap();
        backend.remove(&state, &runner).unwrap();
    } else {
        panic!("no firewall backend available for test");
    }
}
