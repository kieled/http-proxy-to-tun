use std::net::IpAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "proxyvpn",
    version,
    about = "System-wide VPN-like proxy via sing-box TUN"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Up(Box<UpArgs>),
    Down(Box<DownArgs>),
}

#[derive(Args, Debug, Clone)]
pub struct CommonArgs {
    /// State directory (tmpfs on Arch)
    #[arg(long, default_value = "/run/proxyvpn")]
    pub state_dir: PathBuf,

    /// Verbose logging
    #[arg(long)]
    pub verbose: bool,

    /// Keep sing-box logs on teardown
    #[arg(long)]
    pub keep_logs: bool,

    /// Print intended changes without applying
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug, Clone)]
pub struct UpArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Full proxy URL: http://user:pass@host:port
    #[arg(long, conflicts_with_all = ["proxy_host", "proxy_port", "username", "password", "password_file"])]
    pub proxy_url: Option<String>,

    /// Upstream proxy hostname
    #[arg(long)]
    pub proxy_host: Option<String>,

    /// Upstream proxy port
    #[arg(long)]
    pub proxy_port: Option<u16>,

    /// Username for proxy auth
    #[arg(long)]
    pub username: Option<String>,

    /// Password for proxy auth (avoid shell history; prefer --password-file)
    #[arg(long, conflicts_with = "password_file")]
    pub password: Option<String>,

    /// Read password from file
    #[arg(long)]
    pub password_file: Option<PathBuf>,

    /// Explicit proxy IP (repeatable). Skips DNS resolution.
    #[arg(long)]
    pub proxy_ip: Vec<IpAddr>,

    /// TUN interface name
    #[arg(long, default_value = "tun0")]
    pub tun_name: String,

    /// TUN interface CIDR
    #[arg(long, default_value = "172.19.0.1/30")]
    pub tun_cidr: String,

    /// DNS IP to enforce through TUN
    #[arg(long)]
    pub dns: Option<IpAddr>,

    /// Allow DNS queries to these IPs when killswitch is enabled (repeatable)
    #[arg(long)]
    pub allow_dns: Vec<IpAddr>,

    /// Disable firewall killswitch (default: enabled)
    #[arg(long)]
    pub no_killswitch: bool,
}

#[derive(Args, Debug, Clone)]
pub struct DownArgs {
    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn parse_cli_with_default_up() -> Cli {
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        args.insert(1, "up".to_string());
    } else {
        let first = args.get(1).map(String::as_str).unwrap_or("");
        if first != "up" && first != "down" {
            args.insert(1, "up".to_string());
        }
    }
    Cli::parse_from(args)
}

pub fn read_password(args: &UpArgs) -> anyhow::Result<String> {
    if let Some(pw) = &args.password {
        return Ok(pw.clone());
    }
    if let Some(path) = &args.password_file {
        let data = std::fs::read_to_string(path)?;
        return Ok(data.trim_end().to_string());
    }
    anyhow::bail!("missing --password or --password-file");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::net::IpAddr;
    use std::path::PathBuf;

    fn base_common() -> CommonArgs {
        CommonArgs {
            state_dir: PathBuf::from("/tmp/proxyvpn-test"),
            verbose: false,
            keep_logs: false,
            dry_run: false,
        }
    }

    fn base_up_args() -> UpArgs {
        UpArgs {
            common: base_common(),
            proxy_url: None,
            proxy_host: Some("example.com".to_string()),
            proxy_port: Some(8080),
            username: Some("user".to_string()),
            password: None,
            password_file: None,
            proxy_ip: Vec::<IpAddr>::new(),
            tun_name: "tun0".to_string(),
            tun_cidr: "172.19.0.1/30".to_string(),
            dns: None,
            allow_dns: Vec::<IpAddr>::new(),
            no_killswitch: false,
        }
    }

    #[test]
    fn read_password_inline() {
        let mut args = base_up_args();
        args.password = Some("secret".to_string());
        let pw = read_password(&args).unwrap();
        assert_eq!(pw, "secret");
    }

    #[test]
    fn read_password_file() {
        let mut args = base_up_args();
        let dir = std::env::temp_dir();
        let path = dir.join(format!("proxyvpn-pw-{}", std::process::id()));
        fs::write(&path, "file-secret\n").unwrap();
        args.password_file = Some(path.clone());
        let pw = read_password(&args).unwrap();
        assert_eq!(pw, "file-secret");
        let _ = fs::remove_file(path);
    }
}
