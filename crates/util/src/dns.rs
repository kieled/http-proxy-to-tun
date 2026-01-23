use std::net::{IpAddr, Ipv4Addr};

pub fn parse_resolv_conf_str(contents: &str) -> Vec<IpAddr> {
    let mut ips = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("nameserver") {
            let ip_str = rest.trim();
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                ips.push(ip);
            }
        }
    }
    ips
}

pub fn parse_resolv_conf(path: &str) -> Vec<IpAddr> {
    let resolv = std::fs::read_to_string(path).unwrap_or_default();
    parse_resolv_conf_str(&resolv)
}

pub fn first_resolv_conf_v4(path: &str) -> Option<Ipv4Addr> {
    let data = std::fs::read_to_string(path).ok()?;
    for line in data.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("nameserver") {
            let ip_str = rest.trim();
            if let Ok(IpAddr::V4(v4)) = ip_str.parse::<IpAddr>() {
                return Some(v4);
            }
        }
    }
    None
}

pub fn is_loopback(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_resolv_conf_ignores_comments_and_empty() {
        let input = r#"
# comment
nameserver 192.168.1.1

nameserver 1.1.1.1
not_a_nameserver 9.9.9.9
"#;
        let ips = parse_resolv_conf_str(input);
        assert_eq!(ips.len(), 2);
        assert_eq!(ips[0], IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(ips[1], IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
    }

    #[test]
    fn first_resolv_conf_v4_returns_first() {
        let input = "nameserver 127.0.0.1\nnameserver 1.1.1.1\n";
        let dir = std::env::temp_dir();
        let path = dir.join(format!("proxyvpn-resolv-{}", std::process::id()));
        std::fs::write(&path, input).unwrap();
        let ip = first_resolv_conf_v4(path.to_str().unwrap()).unwrap();
        assert_eq!(ip, Ipv4Addr::new(127, 0, 0, 1));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn is_loopback_detects() {
        assert!(is_loopback(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(!is_loopback(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }
}
