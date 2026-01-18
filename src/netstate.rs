use anyhow::Result;

use crate::util::CommandRunner;

pub fn get_default_routes(runner: &CommandRunner) -> Result<Vec<String>> {
    let out = runner.run_capture("ip", &["route", "show", "default"])?;
    let routes = out
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    Ok(routes)
}

pub fn get_sysctl_value(runner: &CommandRunner, key: &str) -> Result<String> {
    runner.run_capture("sysctl", &["-n", key])
}

pub fn restore_default_routes(runner: &CommandRunner, saved: &[String]) -> Result<()> {
    let current = runner.run_capture_allow_fail("ip", &["route", "show", "default"])?;
    for line in current.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let args: Vec<&str> = line.split_whitespace().collect();
        if args.is_empty() {
            continue;
        }
        let mut cmd = vec!["route", "del"];
        cmd.extend(args);
        let _ = runner.run("ip", &cmd);
    }
    for line in saved.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let args: Vec<&str> = line.split_whitespace().collect();
        if args.is_empty() {
            continue;
        }
        let mut cmd = vec!["route", "add"];
        cmd.extend(args);
        let _ = runner.run("ip", &cmd);
    }
    Ok(())
}

pub fn tun_exists(runner: &CommandRunner, tun_name: &str) -> bool {
    runner
        .run_capture_allow_fail("ip", &["link", "show", "dev", tun_name])
        .map(|out| !out.is_empty() && out.contains("UP"))
        .unwrap_or(false)
}

pub fn delete_tun(runner: &CommandRunner, tun_name: &str) -> Result<()> {
    runner.run("ip", &["link", "del", tun_name])
}
