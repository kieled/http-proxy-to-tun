mod iptables;
mod nft;

use std::net::IpAddr;

use anyhow::Result;

use proxyvpn_util::CommandRunner;

pub struct MarkConfig {
    pub mark: u32,
    pub exclude_ips: Vec<IpAddr>,
}

pub enum MarkBackendKind {
    Nft(NftMarkBackend),
    Iptables(IptablesMarkBackend),
}

pub struct NftMarkBackend {
    pub table: String,
    pub chain: String,
}

pub struct IptablesMarkBackend {
    pub chain: String,
}

impl MarkBackendKind {
    pub fn apply(&self, cfg: &MarkConfig, runner: &CommandRunner) -> Result<()> {
        match self {
            MarkBackendKind::Nft(backend) => backend.apply(cfg, runner),
            MarkBackendKind::Iptables(backend) => backend.apply(cfg, runner),
        }
    }

    pub fn remove(&self, runner: &CommandRunner) -> Result<()> {
        match self {
            MarkBackendKind::Nft(backend) => backend.remove(),
            MarkBackendKind::Iptables(backend) => iptables::remove(&backend.chain, runner),
        }
    }
}

impl NftMarkBackend {
    fn apply(&self, cfg: &MarkConfig, runner: &CommandRunner) -> Result<()> {
        // Try native libnftnl first (works with CAP_NET_ADMIN)
        match nft::apply_native(cfg, &self.table, &self.chain) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("native nftnl failed ({e}), falling back to nft command");
                nft::apply(cfg, &self.table, &self.chain, runner)
            }
        }
    }

    fn remove(&self) -> Result<()> {
        nft::delete_table(&self.table)
    }
}

impl IptablesMarkBackend {
    fn apply(&self, cfg: &MarkConfig, runner: &CommandRunner) -> Result<()> {
        iptables::apply(cfg, &self.chain, runner)
    }
}

pub fn choose_mark_backend() -> Result<MarkBackendKind> {
    // Prefer nftables (can use native libnftnl with CAP_NET_ADMIN)
    Ok(MarkBackendKind::Nft(NftMarkBackend {
        table: "proxyvpn_mark".to_string(),
        chain: "output".to_string(),
    }))
}

pub fn remove_mark_rules_best_effort(runner: &CommandRunner) -> Result<()> {
    // Try native removal first (works with CAP_NET_ADMIN)
    match nft::delete_table("proxyvpn_mark") {
        Ok(()) => eprintln!("mark: native nft delete succeeded"),
        Err(e) => eprintln!("mark: native nft delete failed: {e}"),
    }
    // Always try command-based removal as fallback (needs sudo or setcap on nft)
    if proxyvpn_util::is_root() && proxyvpn_util::find_in_path("nft").is_some() {
        let _ = runner.run_capture_allow_fail("nft", &["delete", "table", "inet", "proxyvpn_mark"]);
    }
    // Also try iptables cleanup
    if proxyvpn_util::is_root() && proxyvpn_util::find_in_path("iptables").is_some() {
        let _ = iptables::remove("PROXYVPN_MARK", runner);
    }
    Ok(())
}
