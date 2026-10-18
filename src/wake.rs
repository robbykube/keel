//! Prepare the built-in network port so a later wake packet can turn the desk on.
//!
//! This does not sleep or shut down the computer. Stay reachable remains the
//! posture until a helper on Away can send the wake.

use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

use sysinfo::Networks;

const FLAG: &str = "wake.on";

#[derive(Clone)]
pub struct Nic {
    pub name: String,
    pub mac: String,
    pub house: bool,
}

pub fn view() -> serde_json::Value {
    let mac = wired().map(|nic| nic.mac).unwrap_or_default();
    let ready = wanted();
    let message = if ready && !mac.is_empty() {
        sentence(&mac)
    } else if !cfg!(target_os = "linux") {
        "This preparation runs on the Keel computer. Stay reachable until then.".into()
    } else {
        "Stay reachable. Prepare wake so the built-in port can turn this computer on later. Sleep stays off.".into()
    };
    serde_json::json!({
        "ready": ready,
        "mac": mac,
        "posture": "Stay reachable",
        "message": message,
    })
}

pub fn prepare() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("This preparation runs on the Keel computer.".into());
    }
    let nic = wired().ok_or_else(|| "Keel could not see the built-in network port.".to_string())?;
    let script = script(&nic.name)?;
    crate::storage::host("exec /bin/bash -s", &[], Some(&script))?;
    remember()?;
    Ok(sentence(&nic.mac))
}

pub fn sentence(mac: &str) -> String {
    format!(
        "The wired address is {mac}. This computer can wake from that port. On the EliteDesk startup screen, set Wake On LAN to Boot to Hard Drive, turn S5 Maximum Power Savings off, and set After Power Loss to Power On. Sleep stays off until a Pi on Away can send the wake."
    )
}

pub fn choose(nics: &[Nic]) -> Option<&Nic> {
    nics.iter().find(|nic| {
        nic.house && safe_name(&nic.name) && safe_mac(&nic.mac) && !skipped(&nic.name)
    })
}

pub fn script(iface: &str) -> Result<String, String> {
    if !safe_name(iface) || skipped(iface) {
        return Err("Keel could not see the built-in network port.".into());
    }
    Ok(format!(
        r#"set -uo pipefail
fail() {{ echo "$1" >&2; exit 1; }}
if ! command -v ethtool >/dev/null 2>&1; then
  apt-get update -o Acquire::ForceIPv4=true >/dev/null 2>&1 || true
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true ethtool >/dev/null 2>&1 || fail "Keel could not prepare wake."
fi
tool=$(command -v ethtool)
"$tool" -s {iface} wol g || fail "This port cannot wake the computer. Use the built-in network port."
printf '%s\n' '[Unit]' 'Description=Allow the wired port to wake this computer' 'After=network-online.target' '[Service]' 'Type=oneshot' "ExecStart=$tool -s {iface} wol g" '[Install]' 'WantedBy=multi-user.target' > /etc/systemd/system/keel-wake.service
systemctl daemon-reload || fail "Keel could not remember wake."
systemctl enable keel-wake.service || fail "Keel could not remember wake."
"#
    ))
}

fn wired() -> Option<Nic> {
    choose(&collect()).cloned()
}

fn collect() -> Vec<Nic> {
    let networks = Networks::new_with_refreshed_list();
    networks
        .iter()
        .filter_map(|(name, data)| {
            let mac = data.mac_address().to_string();
            if skipped(name) || !safe_name(name) || !safe_mac(&mac) {
                return None;
            }
            let house = data.ip_networks().iter().any(|net| house_v4(net.addr));
            Some(Nic {
                name: name.to_string(),
                mac,
                house,
            })
        })
        .collect()
}

fn house_v4(addr: IpAddr) -> bool {
    let IpAddr::V4(ip) = addr else {
        return false;
    };
    if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() {
        return false;
    }
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (16..32).contains(&b)) || (a == 192 && b == 168)
}

fn skipped(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "lo"
        || name.starts_with("tailscale")
        || name.starts_with("docker")
        || name.starts_with("veth")
        || name.starts_with("virbr")
        || name.starts_with("br-")
        || name.starts_with("wlan")
        || name.starts_with("wl")
        || name.starts_with("wg")
        || name.starts_with("zt")
        || name.starts_with("tun")
        || name.starts_with("tap")
        || name.starts_with("dummy")
}

pub fn safe_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {
            name.len() <= 15 && chars.all(|c| c.is_ascii_alphanumeric())
        }
        _ => false,
    }
}

pub fn safe_mac(mac: &str) -> bool {
    let parts: Vec<&str> = mac.split(':').collect();
    parts.len() == 6
        && parts.iter().all(|part| {
            part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit())
        })
        && mac != "00:00:00:00:00:00"
}

fn wanted() -> bool {
    flag_path().is_file()
}

fn flag_path() -> PathBuf {
    let root = std::env::var("KEEL_DATA").unwrap_or_else(|_| {
        if cfg!(target_os = "linux") {
            "/var/lib/keel".into()
        } else {
            "data".into()
        }
    });
    PathBuf::from(root).join(FLAG)
}

fn remember() -> Result<(), String> {
    let path = flag_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "Keel prepared wake but could not remember it.".to_string())?;
    }
    fs::write(path, b"on\n").map_err(|_| "Keel prepared wake but could not remember it.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_house_port_is_chosen_and_away_is_not() {
        let nics = [
            Nic {
                name: "tailscale0".into(),
                mac: "02:00:00:00:00:01".into(),
                house: false,
            },
            Nic {
                name: "wlan0".into(),
                mac: "02:00:00:00:00:02".into(),
                house: true,
            },
            Nic {
                name: "enp0s31f6".into(),
                mac: "aa:bb:cc:dd:ee:ff".into(),
                house: true,
            },
        ];
        let chosen = choose(&nics).unwrap();
        assert_eq!(chosen.name, "enp0s31f6");
        assert!(choose(&[nics[0].clone()]).is_none());
    }

    #[test]
    fn the_script_arms_wake_and_does_not_sleep() {
        let text = script("eno1").unwrap();
        assert!(text.contains("ethtool"));
        assert!(text.contains("wol g"));
        assert!(text.contains("eno1"));
        assert!(text.contains("keel-wake.service"));
        assert!(!text.contains("suspend"));
        assert!(!text.contains("poweroff"));
        assert!(script("tailscale0").is_err());
        assert!(script("enp0s31f6;reboot").is_err());
    }
}
