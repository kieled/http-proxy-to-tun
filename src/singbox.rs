use std::net::IpAddr;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};
use serde_json::json;

use crate::state::{open_log_file_0600, write_text_file_0600};

pub struct SingBoxConfig<'a> {
    pub tun_name: &'a str,
    pub tun_cidr: &'a str,
    pub proxy_host: &'a str,
    pub proxy_port: u16,
    pub username: &'a str,
    pub password: &'a str,
    pub dns: Option<IpAddr>,
}

pub struct SingBoxManager {
    pub config_path: std::path::PathBuf,
    pub stdout_path: std::path::PathBuf,
    pub stderr_path: std::path::PathBuf,
}

impl SingBoxManager {
    pub fn write_config(&self, cfg: &SingBoxConfig) -> Result<()> {
        let mut root = json!({
            "inbounds": [
                {
                    "type": "tun",
                    "interface_name": cfg.tun_name,
                    "address": [cfg.tun_cidr],
                    "auto_route": true,
                    "strict_route": true,
                    "sniff": true
                }
            ],
            "outbounds": [
                {
                    "type": "http",
                    "tag": "proxy",
                    "server": cfg.proxy_host,
                    "server_port": cfg.proxy_port,
                    "username": cfg.username,
                    "password": cfg.password
                }
            ],
            "route": {
                "final": "proxy"
            }
        });

        if let Some(dns_ip) = cfg.dns {
            let dns_section = json!({
                "servers": [
                    {
                        "address": dns_ip.to_string()
                    }
                ]
            });
            root.as_object_mut()
                .expect("root object")
                .insert("dns".to_string(), dns_section);
        }

        let data = serde_json::to_string_pretty(&root)?;
        write_text_file_0600(&self.config_path, &data)?;
        Ok(())
    }

    pub fn start(&self) -> Result<Child> {
        let stdout = open_log_file_0600(&self.stdout_path)?;
        let stderr = open_log_file_0600(&self.stderr_path)?;
        let child = Command::new("sing-box")
            .arg("run")
            .arg("-c")
            .arg(&self.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .with_context(|| "failed to start sing-box")?;
        Ok(child)
    }
}
