use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};

pub mod dns;

#[derive(Clone)]
pub struct CommandRunner {
    pub verbose: bool,
    pub dry_run: bool,
}

impl CommandRunner {
    pub fn new(verbose: bool, dry_run: bool) -> Self {
        Self { verbose, dry_run }
    }

    pub fn run(&self, program: &str, args: &[&str]) -> Result<()> {
        if self.verbose {
            eprintln!("$ {} {}", program, args.join(" "));
        }
        if self.dry_run {
            return Ok(());
        }
        let status = Command::new(program)
            .args(args)
            .status()
            .with_context(|| format!("failed to run {program}"))?;
        if !status.success() {
            return Err(anyhow!("command failed: {} {}", program, args.join(" ")));
        }
        Ok(())
    }

    pub fn run_capture_allow_fail(&self, program: &str, args: &[&str]) -> Result<String> {
        if self.verbose {
            eprintln!("$ {} {}", program, args.join(" "));
        }
        if self.dry_run {
            return Ok(String::new());
        }
        let output = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("failed to run {program}"))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

pub fn find_in_path<S: AsRef<OsStr>>(binary: S) -> Option<PathBuf> {
    let binary = binary.as_ref();
    if Path::new(binary).is_file() {
        return Some(PathBuf::from(binary));
    }
    let path_var = std::env::var_os("PATH")?;
    for path in std::env::split_paths(&path_var) {
        let full = path.join(binary);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

pub fn set_permissions_0600(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

pub fn set_permissions_0700(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o700);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

pub fn has_cap_net_admin() -> bool {
    const CAP_NET_ADMIN_BIT: u32 = 12;
    let status = match std::fs::read_to_string("/proc/self/status") {
        Ok(v) => v,
        Err(_) => return false,
    };
    for line in status.lines() {
        if let Some(hex) = line.strip_prefix("CapEff:\t")
            && let Ok(value) = u64::from_str_radix(hex.trim(), 16)
        {
            return (value & (1u64 << CAP_NET_ADMIN_BIT)) != 0;
        }
    }
    false
}

pub fn is_root() -> bool {
    let status = match std::fs::read_to_string("/proc/self/status") {
        Ok(v) => v,
        Err(_) => return false,
    };
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:\t") {
            let mut fields = rest.split_whitespace();
            let _real = fields.next();
            let effective = fields.next();
            return matches!(effective, Some("0"));
        }
    }
    false
}
