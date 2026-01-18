use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, anyhow};

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

    pub fn run_capture(&self, program: &str, args: &[&str]) -> Result<String> {
        if self.verbose {
            eprintln!("$ {} {}", program, args.join(" "));
        }
        let Output {
            status,
            stdout,
            stderr,
        } = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("failed to run {program}"))?;
        if !status.success() {
            return Err(anyhow!(
                "command failed: {} {}\n{}",
                program,
                args.join(" "),
                String::from_utf8_lossy(&stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&stdout).trim().to_string())
    }

    pub fn run_capture_allow_fail(&self, program: &str, args: &[&str]) -> Result<String> {
        if self.verbose {
            eprintln!("$ {} {}", program, args.join(" "));
        }
        let Output { stdout, .. } = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("failed to run {program}"))?;
        Ok(String::from_utf8_lossy(&stdout).trim().to_string())
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
