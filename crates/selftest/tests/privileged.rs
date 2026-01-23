#![cfg(feature = "privileged-tests")]

use std::process::Command;

fn allow_dns_test() -> bool {
    std::env::var("PROXYVPN_PRIV_TESTS_ALLOW_DNS").ok().as_deref() == Some("1")
}

#[test]
#[ignore]
fn dns_probe_binary_works() {
    if !allow_dns_test() {
        eprintln!("skipping selftest dns probe (set PROXYVPN_PRIV_TESTS_ALLOW_DNS=1)");
        return;
    }

    let bin = env!("CARGO_BIN_EXE_proxyvpn-selftest");
    let server = std::env::var("PROXYVPN_PRIV_TESTS_DNS_SERVER").unwrap_or_else(|_| "1.1.1.1".to_string());
    let name = std::env::var("PROXYVPN_PRIV_TESTS_DNS_NAME").unwrap_or_else(|_| "ifconfig.me".to_string());
    let proxy_url = std::env::var("PROXYVPN_PRIV_TESTS_PROXY_URL").ok();

    let mut cmd = Command::new(bin);
    cmd.arg("--server").arg(&server).arg("--name").arg(&name).arg("--no-ip");
    if let Some(url) = proxy_url {
        cmd.arg("--proxy-url").arg(url);
    }

    let output = cmd.output().expect("failed to run selftest");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OK"), "stdout was: {stdout}");
}
