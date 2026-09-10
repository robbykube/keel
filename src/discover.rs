use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Deserialize)]
pub struct LookRequest {
    pub host: String,
}

#[derive(Serialize, Clone)]
pub struct LookHit {
    pub port: u16,
    pub name: String,
    pub open: bool,
    pub hint: String,
    pub fill: String,
}

#[derive(Serialize)]
pub struct LookReport {
    pub host: String,
    pub hits: Vec<LookHit>,
    pub why: String,
}

struct Probe {
    port: u16,
    name: &'static str,
    hint_open: &'static str,
    fill: fn(&str) -> String,
}

fn fill_none(_: &str) -> String {
    String::new()
}

fn fill_smb(host: &str) -> String {
    format!(r"\\{host}\")
}

fn fill_nfs(host: &str) -> String {
    format!("{host}:/")
}

const PROBES: &[Probe] = &[
    Probe {
        port: 8006,
        name: "Proxmox",
        hint_open: "This looks like the Proxmox web door. It is not a file folder. Sign in there to see disks; paste an SMB or NFS path if a guest already shares files.",
        fill: fill_none,
    },
    Probe {
        port: 443,
        name: "Website (https)",
        hint_open: "A locked website. Could be Proxmox behind a front door, or TrueNAS. Sign in below if you have an account there.",
        fill: fill_none,
    },
    Probe {
        port: 80,
        name: "Website",
        hint_open: "A website without the lock. Could be TrueNAS or a files app.",
        fill: fill_none,
    },
    Probe {
        port: 445,
        name: "Windows folders",
        hint_open: "This machine can share folders. The address is \\\\host\\sharename — the share name is still on that machine.",
        fill: fill_smb,
    },
    Probe {
        port: 2049,
        name: "NFS",
        hint_open: "Unix-style folder share. Address looks like host:/path.",
        fill: fill_nfs,
    },
];

pub async fn port_open(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    timeout(Duration::from_millis(700), TcpStream::connect(&addr))
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
}

pub fn normalize_host(raw: &str) -> Result<String, String> {
    let mut s = raw.trim().to_string();
    for prefix in ["https://", "http://", "HTTPS://", "HTTP://"] {
        if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
            s = s[prefix.len()..].to_string();
            break;
        }
    }
    s = s.trim_start_matches('\\').to_string();
    if let Some(i) = s.find('/') {
        s.truncate(i);
    }
    if let Some(i) = s.find('\\') {
        s.truncate(i);
    }
    if let Some(i) = s.rfind(':') {
        let rest = &s[i + 1..];
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            s.truncate(i);
        }
    }
    s = s.trim().trim_matches('[').trim_matches(']').to_string();
    if s.is_empty() {
        return Err("type a computer name or number".into());
    }
    if s.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')) {
        return Err("that does not look like a computer name".into());
    }
    Ok(s)
}

pub async fn look(host: &str) -> Result<LookReport, String> {
    let host = normalize_host(host)?;
    let mut hits = Vec::new();
    for spec in PROBES {
        let open = port_open(&host, spec.port).await;
        hits.push(LookHit {
            port: spec.port,
            name: spec.name.into(),
            open,
            hint: if open {
                spec.hint_open.into()
            } else {
                format!("Nothing on port {}.", spec.port)
            },
            fill: if open { (spec.fill)(&host) } else { String::new() },
        });
    }
    if hits.iter().any(|h| h.open && (h.port == 443 || h.port == 8006 || h.port == 80)) {
        if let Some(hint) = crate::remote::https_hint(&host).await {
            if let Some(h) = hits.iter_mut().find(|h| h.open && (h.port == 443 || h.port == 8006)) {
                h.hint = hint;
            } else if let Some(h) = hits.iter_mut().find(|h| h.open && h.port == 80) {
                h.hint = hint;
            }
        }
    }
    Ok(LookReport {
        host,
        hits,
        why: "We only knock on the address you typed. We do not scan the whole house. Open doors are not the same as a pool — you still paste the folder path, or we fill a starting address when Windows folders or NFS answer.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_host;

    #[test]
    fn strips_url_and_share() {
        assert_eq!(normalize_host("https://rok:8006/").unwrap(), "rok");
        assert_eq!(normalize_host(r"\\truenas\tank").unwrap(), "truenas");
        assert_eq!(normalize_host("192.168.1.10").unwrap(), "192.168.1.10");
    }

    #[test]
    fn rejects_junk() {
        assert!(normalize_host("").is_err());
        assert!(normalize_host("rok and friends").is_err());
    }
}
