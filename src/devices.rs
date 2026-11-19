//! Who answered on the house network. One look, this computer's own address, nothing blocked.

use std::collections::{BTreeMap, BTreeSet};
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
    /// What Keel keeps it by: the hardware address, or the address.
    #[serde(default)]
    pub key: String,
    /// "router", "this", "private" (a phone or tablet with a made-up
    /// hardware address), "raspberry", or "".
    #[serde(default)]
    pub kind: String,
    /// A name the person gave it.
    #[serde(default)]
    pub label: String,
    /// Repair says when a watched device stops answering.
    #[serde(default)]
    pub watched: bool,
    /// Whether it answered in the last look.
    #[serde(default = "yes")]
    pub answering: bool,
    #[serde(default)]
    pub last_seen_ms: u64,
}

fn yes() -> bool {
    true
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
    #[serde(default)]
    gateway: String,
    /// New devices nobody has marked as known yet.
    #[serde(default)]
    unknown: BTreeSet<String>,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    #[serde(default)]
    watched: BTreeSet<String>,
    #[serde(default)]
    last_seen: BTreeMap<String, u64>,
    #[serde(default)]
    looked_ms: u64,
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
    let _one = LOOKING.lock().unwrap_or_else(|e| e.into_inner());
    let text = capture()?;
    let (house, gateway, mut found) = parse_look(&text)?;
    // Devices with no name yet: Apple devices and many Linux ones answer
    // a multicast question about their own address.
    let nameless: Vec<Ipv4Addr> = found.iter().filter(|f| f.name.is_empty()).filter_map(|f| f.address.parse().ok()).collect();
    let answered = multicast_names(&nameless);
    for item in &mut found {
        if item.name.is_empty() {
            if let Some(name) = answered.get(&item.address) {
                item.name = clean_name(name, &item.address);
            }
        }
    }
    let saved = load();
    let (report, next) = merge(&saved, &house, &gateway, found, now_ms());
    store(&next)?;
    Ok(report)
}

static LOOKING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Called about once a minute: a look every ten minutes, so new devices
/// and watched ones that stop answering show in Repair without a press.
pub fn look_if_due() {
    if cfg!(windows) {
        return;
    }
    if now_ms().saturating_sub(load().looked_ms) >= 10 * 60 * 1000 {
        let _ = look();
    }
}

/// A label, known, or watched, for one device. Empty label clears it.
pub fn mark(key: &str, label: Option<&str>, known: Option<bool>, watched: Option<bool>) -> Result<Report, String> {
    let mut saved = load();
    if !saved.seen.iter().any(|k| k == key) {
        return Err("Keel does not know that device. Press Look now.".into());
    }
    if let Some(label) = label {
        let label: String = label.trim().chars().filter(|c| !c.is_control()).take(40).collect();
        if label.is_empty() {
            saved.labels.remove(key);
        } else {
            saved.labels.insert(key.to_string(), label);
        }
    }
    if known == Some(true) {
        saved.unknown.remove(key);
    }
    if let Some(watch) = watched {
        if watch {
            saved.watched.insert(key.to_string());
        } else {
            saved.watched.remove(key);
        }
    }
    for device in &mut saved.devices {
        if device.key == key {
            device.label = saved.labels.get(key).cloned().unwrap_or_default();
            device.new = saved.unknown.contains(key);
            device.watched = saved.watched.contains(key);
        }
    }
    store(&saved)?;
    Ok(saved_report(&saved))
}

fn saved_report(saved: &Saved) -> Report {
    Report { house: saved.house.clone(), devices: saved.devices.clone(), message: saved.message.clone() }
}

/// For Repair: new devices nobody has marked as known, and watched devices
/// that have not answered for ten minutes.
pub fn issues() -> Vec<crate::model::Issue> {
    let saved = load();
    let now = now_ms();
    let mut items = Vec::new();
    let fresh: Vec<&Device> = saved.devices.iter().filter(|d| d.new && !d.here).collect();
    if !fresh.is_empty() {
        items.push(crate::model::Issue {
            id: "device-new",
            severity: "yellow",
            title: if fresh.len() == 1 { "A new device joined the network" } else { "New devices joined the network" },
            why: "Devices shows each one. Mark it as known if you know it; if you do not, change the Wi-Fi password.",
            evidence: fresh.iter().map(|d| describe(d)).collect::<Vec<_>>().join(", "),
            repair: None,
        });
    }
    let gone: Vec<&Device> = saved
        .devices
        .iter()
        .filter(|d| d.watched && !d.answering && now.saturating_sub(d.last_seen_ms) >= 10 * 60 * 1000)
        .collect();
    if !gone.is_empty() {
        items.push(crate::model::Issue {
            id: "device-gone",
            severity: "red",
            title: if gone.len() == 1 { "A watched device stopped answering" } else { "Watched devices stopped answering" },
            why: "Check that it has power and its cable is in. A router or access point that is off leaves part of the house without network.",
            evidence: gone.iter().map(|d| describe(d)).collect::<Vec<_>>().join(", "),
            repair: None,
        });
    }
    items
}

fn describe(device: &Device) -> String {
    let name = if !device.label.is_empty() {
        device.label.clone()
    } else if !device.name.is_empty() {
        device.name.clone()
    } else {
        device.address.clone()
    };
    if name == device.address { name } else { format!("{name} ({})", device.address) }
}

fn capture() -> Result<String, String> {
    #[cfg(not(windows))]
    {
        // The look prints one line per device. host_text keeps only the last
        // line, which is never the house address, so the look would always
        // say the network could not be seen.
        crate::storage::host_all("exec /bin/bash -s", &[], Some(LOOK_SH))
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

fn merge(saved: &Saved, house: &str, gateway: &str, found: Vec<Found>, now: u64) -> (Report, Saved) {
    let known: BTreeSet<&str> = saved.seen.iter().map(String::as_str).collect();
    let had = !known.is_empty();
    let mut devices = Vec::new();
    let mut seen = known.iter().map(|key| (*key).to_string()).collect::<BTreeSet<_>>();
    let mut seen_self = false;
    let mut unknown = saved.unknown.clone();
    let mut last_seen = saved.last_seen.clone();
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
        if fresh && !here {
            unknown.insert(key.clone());
        }
        seen.insert(key.clone());
        last_seen.insert(key.clone(), now);
        let kind = kind_of(&item.address, &item.mac, here, gateway);
        devices.push(Device {
            name: if here {
                "This computer".into()
            } else {
                item.name
            },
            new: unknown.contains(&key) && !here,
            label: saved.labels.get(&key).cloned().unwrap_or_default(),
            watched: saved.watched.contains(&key),
            answering: true,
            last_seen_ms: now,
            kind: kind.into(),
            key,
            address: item.address,
            mac: item.mac,
            here,
        });
    }
    // A watched device that did not answer stays on the list, so Repair
    // can say so.
    for old in &saved.devices {
        if old.watched && !old.key.is_empty() && !devices.iter().any(|d| d.key == old.key) {
            let mut kept = old.clone();
            kept.answering = false;
            kept.last_seen_ms = last_seen.get(&old.key).copied().unwrap_or(old.last_seen_ms);
            devices.push(kept);
        }
    }
    if !seen_self && is_house_ipv4(house) {
        let key = house.to_string();
        seen.insert(key.clone());
        devices.push(Device {
            address: house.to_string(),
            mac: String::new(),
            name: "This computer".into(),
            here: true,
            new: false,
            key,
            kind: "this".into(),
            label: String::new(),
            watched: false,
            answering: true,
            last_seen_ms: now,
        });
    }
    devices.sort_by(|a, b| ip_key(&a.address).cmp(&ip_key(&b.address)));
    let message = message_for(devices.len(), devices.iter().filter(|d| d.new).count(), had);
    let report = Report {
        house: house.to_string(),
        devices: devices.clone(),
        message: message.clone(),
    };
    unknown.retain(|key| seen.contains(key));
    let next = Saved {
        seen: seen.into_iter().collect(),
        house: house.to_string(),
        devices,
        message,
        gateway: gateway.to_string(),
        unknown,
        labels: saved.labels.clone(),
        watched: saved.watched.clone(),
        last_seen,
        looked_ms: now,
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

/// What kind of device, from what Keel can tell without asking it.
fn kind_of(address: &str, mac: &str, here: bool, gateway: &str) -> &'static str {
    if here {
        return "this";
    }
    if !gateway.is_empty() && address == gateway {
        return "router";
    }
    // The second hex digit 2, 6, a or e: a made-up address, as phones and
    // tablets use on each Wi-Fi network.
    if let Some(second) = mac.chars().nth(1) {
        if matches!(second, '2' | '6' | 'a' | 'e') {
            return "private";
        }
    }
    const PI: [&str; 5] = ["b8:27:eb", "dc:a6:32", "e4:5f:01", "d8:3a:dd", "2c:cf:67"];
    if PI.iter().any(|oui| mac.starts_with(oui)) {
        return "raspberry";
    }
    ""
}

/// Asks the local multicast group (224.0.0.251:5353) who each address is,
/// with the "answer me directly" bit, and waits a moment for replies.
fn multicast_names(addresses: &[Ipv4Addr]) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    if addresses.is_empty() || cfg!(windows) {
        return names;
    }
    let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") else {
        return names;
    };
    let _ = socket.set_read_timeout(Some(std::time::Duration::from_millis(250)));
    for (n, address) in addresses.iter().enumerate() {
        let [a, b, c, d] = address.octets();
        let query = ptr_query(n as u16, &format!("{d}.{c}.{b}.{a}.in-addr.arpa"));
        let _ = socket.send_to(&query, "224.0.0.251:5353");
    }
    let until = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    let mut buf = [0u8; 1500];
    while std::time::Instant::now() < until {
        let Ok((len, _)) = socket.recv_from(&mut buf) else {
            continue;
        };
        if let Some((address, name)) = ptr_answer(&buf[..len]) {
            names.insert(address, name.trim_end_matches(".local").to_string());
        }
    }
    names
}

fn ptr_query(id: u16, name: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(64);
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&[0, 12]); // PTR
    q.extend_from_slice(&[0x80, 1]); // IN, answer directly
    q
}

/// The first PTR answer for an in-addr.arpa name: ("192.168.0.5", "iPhone").
fn ptr_answer(msg: &[u8]) -> Option<(String, String)> {
    if msg.len() < 12 || msg[2] & 0x80 == 0 {
        return None;
    }
    let questions = u16::from_be_bytes([msg[4], msg[5]]) as usize;
    let answers = u16::from_be_bytes([msg[6], msg[7]]) as usize;
    let mut at = 12;
    for _ in 0..questions {
        let (_, next) = read_name(msg, at)?;
        at = next + 4;
    }
    for _ in 0..answers {
        let (owner, next) = read_name(msg, at)?;
        let kind = u16::from_be_bytes([*msg.get(next)?, *msg.get(next + 1)?]);
        let len = u16::from_be_bytes([*msg.get(next + 8)?, *msg.get(next + 9)?]) as usize;
        let data = next + 10;
        if kind == 12 && owner.ends_with(".in-addr.arpa") {
            let (target, _) = read_name(msg, data)?;
            let parts: Vec<&str> = owner.trim_end_matches(".in-addr.arpa").split('.').collect();
            if parts.len() == 4 {
                return Some((format!("{}.{}.{}.{}", parts[3], parts[2], parts[1], parts[0]), target));
            }
        }
        at = data + len;
    }
    None
}

fn read_name(msg: &[u8], mut at: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut end = None;
    for _ in 0..64 {
        let len = *msg.get(at)? as usize;
        if len == 0 {
            return Some((labels.join("."), end.unwrap_or(at + 1)));
        }
        if len & 0xc0 == 0xc0 {
            let pointer = ((len & 0x3f) << 8) | *msg.get(at + 1)? as usize;
            end.get_or_insert(at + 2);
            at = pointer;
            continue;
        }
        labels.push(String::from_utf8_lossy(msg.get(at + 1..at + 1 + len)?).to_string());
        at += 1 + len;
    }
    None
}

fn key_of(mac: &str, address: &str) -> String {
    if mac.is_empty() {
        address.to_string()
    } else {
        mac.to_string()
    }
}

/// Lines from the look: `house|address`, `gateway|address` and `address|mac|name`.
fn parse_look(text: &str) -> Result<(String, String, Vec<Found>), String> {
    let mut house = None;
    let mut gateway = String::new();
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
        if first == "gateway" {
            if is_house_ipv4(second) {
                gateway = second.to_string();
            }
            continue;
        }
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
    Ok((house, gateway, found))
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
        let (house, gateway, found) = parse_look(text).unwrap();
        assert_eq!(house, "192.168.0.20");
        assert_eq!(gateway, "");
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
        let (report, saved) = merge(&Saved::default(), "192.168.0.20", "192.168.0.1", found, 1_000);
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
        let (report, _) = merge(&saved, "192.168.0.20", "192.168.0.1", found, 1_000);
        let known = report.devices.iter().find(|d| d.address == "192.168.0.8").unwrap();
        let fresh = report.devices.iter().find(|d| d.address == "192.168.0.40").unwrap();
        assert!(!known.new);
        assert!(fresh.new);
        assert_eq!(report.message, "3 devices answered. 1 is new.");
    }

    #[test]
    fn a_new_device_stays_new_until_marked_and_a_watched_one_is_missed() {
        let mut saved = Saved { seen: vec!["aa:bb:cc:dd:ee:ff".into()], ..Saved::default() };
        let router = || Found { address: "192.168.0.1".into(), mac: "aa:bb:cc:dd:ee:ff".into(), name: String::new() };
        let phone = || Found { address: "192.168.0.50".into(), mac: "3a:11:22:33:44:55".into(), name: "iPhone".into() };
        let (report, next) = merge(&saved, "192.168.0.20", "192.168.0.1", vec![router(), phone()], 1_000);
        let found = |r: &Report, a: &str| r.devices.iter().find(|d| d.address == a).cloned().unwrap();
        assert_eq!(found(&report, "192.168.0.1").kind, "router");
        assert_eq!(found(&report, "192.168.0.50").kind, "private");
        assert!(found(&report, "192.168.0.50").new);
        // Still new at the next look, until someone marks it as known.
        let (again, next) = merge(&next, "192.168.0.20", "192.168.0.1", vec![router(), phone()], 2_000);
        assert!(found(&again, "192.168.0.50").new);
        saved = next;
        saved.unknown.remove("3a:11:22:33:44:55");
        saved.watched.insert("aa:bb:cc:dd:ee:ff".into());
        saved.devices.iter_mut().for_each(|d| if d.key == "aa:bb:cc:dd:ee:ff" { d.watched = true; });
        // The router does not answer: it stays, marked as not answering.
        let (later, _) = merge(&saved, "192.168.0.20", "192.168.0.1", vec![phone()], 3_000);
        let router_now = found(&later, "192.168.0.1");
        assert!(!router_now.answering);
        assert_eq!(router_now.last_seen_ms, 2_000);
        assert!(!found(&later, "192.168.0.50").new);
    }

    #[test]
    fn a_multicast_answer_names_the_address() {
        let query = ptr_query(7, "50.0.168.192.in-addr.arpa");
        assert_eq!(&query[..2], &[0, 7]);
        // An answer: the question echoed, then a PTR to "iPhone.local".
        let mut answer = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        let owner = &query[12..query.len() - 4];
        answer.extend_from_slice(owner);
        answer.extend_from_slice(&[0, 12, 0x80, 1, 0, 0, 0, 120]);
        let target = b"\x06iPhone\x05local\x00";
        answer.extend_from_slice(&(target.len() as u16).to_be_bytes());
        answer.extend_from_slice(target);
        assert_eq!(ptr_answer(&answer), Some(("192.168.0.50".into(), "iPhone.local".into())));
        assert_eq!(ptr_answer(&[0; 4]), None);
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
        assert!(LOOK_SH.matches("printf 'house|%s\\n'").count() >= 2);
        assert!(!LOOK_SH.contains("0.0.0.0/0"));
        assert!(!LOOK_SH.contains("nmap"));
    }
}
