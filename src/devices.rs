//! Who answered on the house network. One look, this computer's own address, nothing blocked.

use std::collections::BTreeSet;
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg_attr(windows, allow(dead_code))]
const LOOK_SH: &str = include_str!("devices-look.sh");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub address: String,
    pub mac: String,
    pub name: String,
    pub here: bool,
    pub new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Report {
    pub house: String,
    pub devices: Vec<Device>,
    pub message: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    seen: Vec<String>,
    house: String,
    devices: Vec<Device>,
    message: String,
}

struct Found {
    address: String,
    mac: String,
    name: String,
}

pub fn saved() -> Report {
    let saved = load();
    if saved.devices.is_empty() {
        return Report {
            house: saved.house,
            devices: Vec::new(),
            message: "Press Look now.".into(),
        };
    }
    Report {
        house: saved.house,
        devices: saved.devices,
        message: saved.message,
    }
}

pub fn look() -> Result<Report, String> {
    let text = capture()?;
    let (house, found) = parse_look(&text)?;
    let saved = load();
    let (report, next) = merge(&saved, &house, found);
    store(&next)?;
    Ok(report)
}

fn capture() -> Result<String, String> {
    #[cfg(not(windows))]
    {
        crate::storage::host_text("exec /bin/bash -s", LOOK_SH)
    }
    #[cfg(windows)]
    {
        windows_capture()
    }
}

fn load() -> Saved {
    let Ok(text) = fs::read_to_string(store_path()) else {
        return Saved::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn store(saved: &Saved) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "Keel could not remember the list.".to_string())?;
    }
    let text = serde_json::to_string_pretty(saved)
        .map_err(|_| "Keel could not remember the list.".to_string())?;
    fs::write(path, text).map_err(|_| "Keel could not remember the list.".to_string())
}

fn store_path() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join("devices.json")
}

fn merge(saved: &Saved, house: &str, found: Vec<Found>) -> (Report, Saved) {
    let known: BTreeSet<&str> = saved.seen.iter().map(String::as_str).collect();
    let had = !known.is_empty();
    let mut devices = Vec::new();
    let mut seen = known.iter().map(|key| (*key).to_string()).collect::<BTreeSet<_>>();
    let mut seen_self = false;
    for item in found {
        if !same_house(house, &item.address) {
            continue;
        }
        let here = item.address == house;
        if here {
            seen_self = true;
        }
        let key = key_of(&item.mac, &item.address);
        let fresh = had && !known.contains(key.as_str());
        seen.insert(key);
        devices.push(Device {
            name: if here {
                "This computer".into()
            } else {
                item.name
            },
            address: item.address,
            mac: item.mac,
            here,
            new: fresh && !here,
        });
    }
    if !seen_self && is_house_ipv4(house) {
        let key = house.to_string();
        seen.insert(key);
        devices.push(Device {
            address: house.to_string(),
            mac: String::new(),
            name: "This computer".into(),
            here: true,
            new: false,
        });
    }
    devices.sort_by(|a, b| ip_key(&a.address).cmp(&ip_key(&b.address)));
    let message = message_for(devices.len(), devices.iter().filter(|d| d.new).count(), had);
    let report = Report {
        house: house.to_string(),
        devices: devices.clone(),
        message: message.clone(),
    };
    let next = Saved {
        seen: seen.into_iter().collect(),
        house: house.to_string(),
        devices,
        message,
    };
    (report, next)
}

fn message_for(count: usize, fresh: usize, had: bool) -> String {
    if count == 0 {
        return "Nobody answered.".into();
    }
    let noun = if count == 1 { "device" } else { "devices" };
    if !had {
        return format!("{count} {noun} answered. This is the first list.");
    }
    if fresh == 0 {
        return format!("{count} {noun} answered. None are new.");
    }
    if fresh == 1 {
        return format!("{count} {noun} answered. 1 is new.");
    }
    format!("{count} {noun} answered. {fresh} are new.")
}

fn key_of(mac: &str, address: &str) -> String {
    if mac.is_empty() {
        address.to_string()
    } else {
        mac.to_string()
    }
}

/// Lines from the look: `house|address` and `address|mac|name`.
fn parse_look(text: &str) -> Result<(String, Vec<Found>), String> {
    let mut house = None;
    let mut found = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('|');
        let first = parts.next().unwrap_or("");
        let second = parts.next().unwrap_or("");
        let third = parts.next().unwrap_or("");
        if first == "house" {
            if is_house_ipv4(second) {
                house = Some(second.to_string());
            }
            continue;
        }
        if !is_house_ipv4(first) {
            continue;
        }
        let mac = normalize_mac(second);
        if !mac.is_empty() && !mac_ok(&mac) {
            continue;
        }
        found.push(Found {
            address: first.to_string(),
            mac,
            name: clean_name(third, first),
        });
    }
    let house = house.ok_or_else(|| "Keel could not see the house network.".to_string())?;
    Ok((house, found))
}

fn is_house_ipv4(text: &str) -> bool {
    let Some(ip) = text.parse::<Ipv4Addr>().ok() else {
        return false;
    };
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 192 && b == 168) || (a == 172 && (16..=31).contains(&b))
}

fn same_house(house: &str, address: &str) -> bool {
    house.rsplit_once('.').map(|(prefix, _)| prefix)
        == address.rsplit_once('.').map(|(prefix, _)| prefix)
        && is_house_ipv4(address)
}

fn normalize_mac(text: &str) -> String {
    text.trim().replace('-', ":").to_ascii_lowercase()
}

fn mac_ok(text: &str) -> bool {
    let parts: Vec<&str> = text.split(':').collect();
    parts.len() == 6
        && parts.iter().all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()))
        && text != "ff:ff:ff:ff:ff:ff"
        && !text.starts_with("01:00:5e:")
}

fn clean_name(text: &str, address: &str) -> String {
    let name = text.trim().trim_end_matches('.').to_string();
    if name.is_empty() || name == address || name.len() > 63 {
        return String::new();
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
    {
        return String::new();
    }
    name
}

fn ip_key(address: &str) -> [u8; 4] {
    address
        .parse::<Ipv4Addr>()
        .map(|ip| ip.octets())
        .unwrap_or([255, 255, 255, 255])
}

#[cfg(windows)]
fn windows_capture() -> Result<String, String> {
    let route = command_text("route", &["print", "-4"])?;
    let house = interface_from_route(&route)
        .ok_or_else(|| "Keel could not see the house network.".to_string())?;
    let prefix = house
        .rsplit_once('.')
        .map(|(prefix, _)| prefix.to_string())
        .ok_or_else(|| "Keel could not see the house network.".to_string())?;
    let mut lines = vec![format!("house|{house}")];
    let replies = windows_ping(&prefix);
    let arp = command_text("arp", &["-a"]).unwrap_or_default();
    let macs = macs_from_arp(&arp, &prefix);
    for (address, name) in &replies {
        if !same_house(&house, address) {
            continue;
        }
        let mac = macs.get(address.as_str()).cloned().unwrap_or_default();
        lines.push(format!("{address}|{mac}|{name}"));
    }
    for (address, mac) in &macs {
        if replies.iter().any(|(ip, _)| ip == address) {
            continue;
        }
        lines.push(format!("{address}|{mac}|"));
    }
    Ok(lines.join("\n"))
}

#[cfg(windows)]
fn windows_ping(prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = Vec::new();
    for host in 1..=254 {
        let address = format!("{prefix}.{host}");
        let child = std::process::Command::new("ping")
            .args(["-n", "1", "-w", "400", &address])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        let Ok(child) = child else { continue };
        pending.push((address, child));
        if pending.len() >= 32 {
            out.extend(take_pings(&mut pending));
        }
    }
    out.extend(take_pings(&mut pending));
    out
}

#[cfg(windows)]
fn take_pings(pending: &mut Vec<(String, std::process::Child)>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (address, child) in pending.drain(..) {
        let Ok(done) = child.wait_with_output() else { continue };
        if !done.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&done.stdout);
        let lower = text.to_ascii_lowercase();
        if !lower.contains("reply from") || !lower.contains("ttl=") || lower.contains("unreachable") {
            continue;
        }
        out.push((address.clone(), name_from_ping(&text, &address)));
    }
    out
}

#[cfg(windows)]
fn command_text(program: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|_| "Keel could not see the house network.".to_string())?;
    if !output.status.success() {
        return Err("Keel could not see the house network.".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn interface_from_route(text: &str) -> Option<String> {
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "0.0.0.0" && parts[1] == "0.0.0.0" && is_house_ipv4(parts[3])
        {
            return Some(parts[3].to_string());
        }
    }
    None
}

fn name_from_ping(text: &str, address: &str) -> String {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Pinging ") else { continue };
        let Some((name, _)) = rest.split_once(" [") else { continue };
        return clean_name(name, address);
    }
    String::new()
}

fn macs_from_arp(text: &str, prefix: &str) -> std::collections::BTreeMap<String, String> {
    let mut macs = std::collections::BTreeMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let address = parts[0];
        if !address.starts_with(&format!("{prefix}.")) || !is_house_ipv4(address) {
            continue;
        }
        let mac = normalize_mac(parts[1]);
        if mac_ok(&mac) {
            macs.insert(address.to_string(), mac);
        }
    }
    macs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn house_address_stays_private() {
        assert!(is_house_ipv4("192.168.1.20"));
        assert!(is_house_ipv4("10.1.2.3"));
        assert!(is_house_ipv4("172.16.5.9"));
        assert!(!is_house_ipv4("100.64.1.2"));
        assert!(!is_house_ipv4("8.8.8.8"));
        assert!(!is_house_ipv4("127.0.0.1"));
    }

    #[test]
    fn route_picks_the_house_interface() {
        let text = "\
          0.0.0.0          0.0.0.0      192.168.0.1     192.168.0.20     25\n\
          0.0.0.0          0.0.0.0      100.64.0.1      100.99.1.4       5\n";
        assert_eq!(interface_from_route(text).as_deref(), Some("192.168.0.20"));
        assert_eq!(interface_from_route("0.0.0.0 0.0.0.0 1.1.1.1 8.8.8.8 1"), None);
    }

    #[test]
    fn neigh_lines_keep_a_name_and_skip_a_miss() {
        let text = "\
house|192.168.0.20
192.168.0.1|AA-BB-CC-DD-EE-FF|router
192.168.0.9|ff:ff:ff:ff:ff:ff|
8.8.8.8|aa:bb:cc:dd:ee:01|public
";
        let (house, found) = parse_look(text).unwrap();
        assert_eq!(house, "192.168.0.20");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].mac, "aa:bb:cc:dd:ee:ff");
        assert_eq!(found[0].name, "router");
    }

    #[test]
    fn first_list_is_not_all_new() {
        let found = vec![Found {
            address: "192.168.0.1".into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
            name: "router".into(),
        }];
        let (report, saved) = merge(&Saved::default(), "192.168.0.20", found);
        assert!(!report.devices.iter().any(|device| device.new));
        assert!(report.message.contains("first list"));
        assert!(saved.seen.iter().any(|key| key == "aa:bb:cc:dd:ee:ff"));
    }

    #[test]
    fn a_new_mac_is_marked_once() {
        let saved = Saved {
            seen: vec!["aa:bb:cc:dd:ee:ff".into()],
            ..Saved::default()
        };
        let found = vec![
            Found {
                address: "192.168.0.8".into(),
                mac: "aa:bb:cc:dd:ee:ff".into(),
                name: String::new(),
            },
            Found {
                address: "192.168.0.40".into(),
                mac: "11:22:33:44:55:66".into(),
                name: String::new(),
            },
        ];
        let (report, _) = merge(&saved, "192.168.0.20", found);
        let known = report.devices.iter().find(|d| d.address == "192.168.0.8").unwrap();
        let fresh = report.devices.iter().find(|d| d.address == "192.168.0.40").unwrap();
        assert!(!known.new);
        assert!(fresh.new);
        assert_eq!(report.message, "3 devices answered. 1 is new.");
    }

    #[test]
    fn ping_and_arp_samples() {
        let name = name_from_ping("Pinging living-room [192.168.0.15] with 32 bytes of data:", "192.168.0.15");
        assert_eq!(name, "living-room");
        assert_eq!(name_from_ping("Pinging 192.168.0.15 [192.168.0.15] with 32 bytes:", "192.168.0.15"), "");
        let arp = "192.168.0.1           00-11-22-33-44-55     dynamic\n192.168.0.255         ff-ff-ff-ff-ff-ff     static\n";
        let macs = macs_from_arp(arp, "192.168.0");
        assert_eq!(macs.get("192.168.0.1").map(String::as_str), Some("00:11:22:33:44:55"));
        assert!(!macs.contains_key("192.168.0.255"));
    }

    #[test]
    fn look_script_uses_this_computer_only() {
        assert!(LOOK_SH.contains("ip -4 route get 1.1.1.1"));
        assert!(LOOK_SH.contains("ping -c 1 -W 1"));
        assert!(LOOK_SH.contains("ip -4 neigh show"));
        assert!(!LOOK_SH.contains("0.0.0.0/0"));
        assert!(!LOOK_SH.contains("nmap"));
    }
}
