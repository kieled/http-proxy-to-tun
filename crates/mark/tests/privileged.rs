#![cfg(feature = "privileged-tests")]

use proxyvpn_mark::{MarkBackendKind, MarkConfig, choose_mark_backend, remove_mark_rules_best_effort};
use proxyvpn_util::CommandRunner;

fn allow_mark_tests() -> bool {
    std::env::var("PROXYVPN_PRIV_TESTS_ALLOW_MARK").ok().as_deref() == Some("1")
}

#[test]
#[ignore]
fn apply_and_remove_mark_rules() {
    if !allow_mark_tests() {
        eprintln!("skipping mark test (set PROXYVPN_PRIV_TESTS_ALLOW_MARK=1)");
        return;
    }

    let backend = choose_mark_backend().unwrap();
    let runner = CommandRunner::new(true, false);
    backend
        .apply(
            &MarkConfig {
                mark: 0x1,
                exclude_ips: Vec::new(),
            },
            &runner,
        )
        .unwrap();
    remove_mark_rules_best_effort(&runner).unwrap();

    if let MarkBackendKind::Iptables(_) = backend {
        // no-op: just ensure enum is used to avoid warnings in tests
    }
}
