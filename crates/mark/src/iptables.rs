use anyhow::Result;

use proxyvpn_util::CommandRunner;

use crate::MarkConfig;

pub(crate) fn build_commands(cfg: &MarkConfig, chain: &str) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    cmds.push(vec!["-t", "mangle", "-D", "OUTPUT", "-j", chain].into_iter().map(String::from).collect());
    cmds.push(vec!["-t", "mangle", "-F", chain].into_iter().map(String::from).collect());
    cmds.push(vec!["-t", "mangle", "-X", chain].into_iter().map(String::from).collect());
    cmds.push(vec!["-t", "mangle", "-N", chain].into_iter().map(String::from).collect());
    for ip in cfg.exclude_ips.iter().filter(|ip| ip.is_ipv4()) {
        cmds.push(vec![
            "-t".to_string(),
            "mangle".to_string(),
            "-A".to_string(),
            chain.to_string(),
            "-d".to_string(),
            ip.to_string(),
            "-j".to_string(),
            "RETURN".to_string(),
        ]);
    }
    cmds.push(vec![
        "-t".to_string(),
        "mangle".to_string(),
        "-A".to_string(),
        chain.to_string(),
        "-p".to_string(),
        "tcp".to_string(),
        "-j".to_string(),
        "MARK".to_string(),
        "--set-mark".to_string(),
        cfg.mark.to_string(),
    ]);
    cmds.push(vec!["-t", "mangle", "-I", "OUTPUT", "1", "-j", chain].into_iter().map(String::from).collect());
    cmds
}

pub(crate) fn apply(cfg: &MarkConfig, chain: &str, runner: &CommandRunner) -> Result<()> {
    let cmds = build_commands(cfg, chain);
    for (idx, cmd) in cmds.into_iter().enumerate() {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        if idx < 3 {
            let _ = runner.run_capture_allow_fail("iptables", &args);
        } else {
            runner.run("iptables", &args)?;
        }
    }
    Ok(())
}

pub(crate) fn remove(chain: &str, runner: &CommandRunner) -> Result<()> {
    let _ = runner.run_capture_allow_fail("iptables", &["-t", "mangle", "-D", "OUTPUT", "-j", chain]);
    let _ = runner.run_capture_allow_fail("iptables", &["-t", "mangle", "-F", chain]);
    let _ = runner.run_capture_allow_fail("iptables", &["-t", "mangle", "-X", chain]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_commands_sets_mark() {
        let cfg = MarkConfig {
            mark: 0x1,
            exclude_ips: Vec::new(),
        };
        let cmds = build_commands(&cfg, "PROXYVPN_MARK");
        let mark_cmd = cmds
            .iter()
            .find(|cmd| cmd.contains(&"--set-mark".to_string()))
            .expect("missing mark rule");
        assert!(mark_cmd.contains(&"1".to_string()));
    }

    #[test]
    fn build_commands_includes_excludes() {
        let cfg = MarkConfig {
            mark: 0x1,
            exclude_ips: vec!["1.2.3.4".parse().unwrap()],
        };
        let cmds = build_commands(&cfg, "PROXYVPN_MARK");
        assert!(cmds.iter().any(|cmd| cmd.contains(&"RETURN".to_string())));
    }
}
