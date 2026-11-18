//! From outside the house.
//!
//! People outside the house reach the apps at the name you own through the
//! router. Keel checks whether this house has its own address on the internet,
//! and can ask the router to send the app ports here. Most routers take that
//! request from a computer in the house without a password (UPnP, or NAT-PMP
//! on some). The request lasts an hour, and Keel renews it while it is on.
//! Only the three ports the apps need are asked for.

use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The doors the apps need: the front door for every app name, and the call.
pub const PORTS: &[(Proto, u16)] = &[
    (Proto::Tcp, 80),
    (Proto::Tcp, 443),
    // Calls: one port for the full Video, twenty for the light one.
    (Proto::Udp, 10000), (Proto::Udp, 10001), (Proto::Udp, 10002), (Proto::Udp, 10003),
    (Proto::Udp, 10004), (Proto::Udp, 10005), (Proto::Udp, 10006), (Proto::Udp, 10007),
    (Proto::Udp, 10008), (Proto::Udp, 10009), (Proto::Udp, 10010), (Proto::Udp, 10011),
    (Proto::Udp, 10012), (Proto::Udp, 10013), (Proto::Udp, 10014), (Proto::Udp, 10015),
    (Proto::Udp, 10016), (Proto::Udp, 10017), (Proto::Udp, 10018), (Proto::Udp, 10019),
];
/// How long one request lasts. Keel asks again well before it runs out.
const LEASE_SECONDS: u32 = 3600;
pub const RENEW_EVERY: Duration = Duration::from_secs(20 * 60);
const DESCRIPTION: &str = "Keel";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    fn word(self) -> &'static str {
        match self {
            Proto::Tcp => "TCP",
            Proto::Udp => "UDP",
        }
    }
}

#[derive(Serialize, Default)]
pub struct View {
    /// open: this house has its own address and the router can send calls
    /// here. shared: the address is shared, so forwarding cannot help.
    /// unknown: Keel could not tell. unavailable: not on the Keel computer.
    pub state: &'static str,
    pub sentence: String,
    pub public: String,
    pub router_outside: String,
    /// How Keel talks to the router: UPnP or NAT-PMP, or empty.
    pub method: String,
    /// Keel is asking the router, and renews the request.
    pub asked: bool,
    pub forwarded: Vec<String>,
    pub note: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    asked: bool,
    #[serde(default)]
    method: String,
    #[serde(default)]
    forwarded: Vec<String>,
    #[serde(default)]
    note: String,
}

fn saved_path() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join("router.json")
}

fn load() -> Saved {
    fs::read_to_string(saved_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn store(saved: &Saved) {
    let path = saved_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(saved) {
        let _ = fs::write(path, text);
    }
}

/// Keel is asking the router for the ports, so the firewall lets them in.
pub fn wanted() -> bool {
    load().asked
}

/// Looks at the router and the house's address on the internet.
/// `public` is the address the outside world sees, found by the caller.
pub fn check(public: Option<Ipv4Addr>) -> View {
    let saved = load();
    let mut view = View {
        asked: saved.asked,
        forwarded: saved.forwarded.clone(),
        note: saved.note.clone(),
        public: public.map(|ip| ip.to_string()).unwrap_or_default(),
        ..View::default()
    };
    if !cfg!(target_os = "linux") {
        view.state = "unavailable";
        view.sentence = "This check runs on the Keel computer.".into();
        return view;
    }
    let (outside, method) = match router_outside() {
        Some((ip, method)) => (Some(ip), method),
        None => (None, ""),
    };
    view.router_outside = outside.map(|ip| ip.to_string()).unwrap_or_default();
    view.method = method.into();
    let (state, sentence) = classify(outside, public, method);
    view.state = state;
    view.sentence = sentence;
    view
}

/// What the two addresses say about calls from outside.
pub fn classify(outside: Option<Ipv4Addr>, public: Option<Ipv4Addr>, method: &str) -> (&'static str, String) {
    match (outside, public) {
        (Some(outside), _) if is_cgnat(outside) => (
            "shared",
            format!(
                "The internet provider shares one address among many homes (the router's outside address is {outside}). The router cannot send calls here, so people outside the house cannot reach the apps this way. Away still works."
            ),
        ),
        (Some(outside), _) if outside.is_private() => (
            "shared",
            format!(
                "There are two routers in a row: this one gets the address {outside} from another box. Both would have to send the ports here. Away still works."
            ),
        ),
        (Some(outside), Some(public)) if outside == public => (
            "open",
            format!(
                "This house has its own address on the internet, {public}. The router can send calls and the app names here. It answers Keel through {method}."
            ),
        ),
        (Some(outside), Some(public)) => (
            "shared",
            format!(
                "The router's outside address is {outside}, but the internet sees this house as {public}. The provider is in between, so the router cannot send calls here. Away still works."
            ),
        ),
        (Some(outside), None) => (
            "unknown",
            format!(
                "The router's outside address is {outside}. Keel could not reach the internet to compare it. Try again when the internet is up."
            ),
        ),
        (None, Some(public)) => (
            "unknown",
            format!(
                "The internet sees this house as {public}. The router did not answer Keel, so Keel cannot ask it for the ports. Its settings page can send TCP 80 and 443 and UDP 10000-10019 to this computer by hand."
            ),
        ),
        (None, None) => (
            "unknown",
            "Keel could not reach the router or the internet to check. Try again when the internet is up.".into(),
        ),
    }
}

fn is_cgnat(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 100 && (64..128).contains(&b)
}

/// The router's address on the internet side, and how Keel reached it.
fn router_outside() -> Option<(Ipv4Addr, &'static str)> {
    if let Ok(gateway) = igd_next::search_gateway(search_options()) {
        if let Ok(IpAddr::V4(ip)) = gateway.get_external_ip() {
            return Some((ip, "UPnP"));
        }
    }
    let gateway = default_gateway()?;
    natpmp_public(gateway).map(|ip| (ip, "NAT-PMP"))
}

fn search_options() -> igd_next::SearchOptions {
    igd_next::SearchOptions {
        timeout: Some(Duration::from_secs(3)),
        ..Default::default()
    }
}

/// Asks the router to send the app ports to this computer. Each port is
/// tried; the answer lists the ones the router took.
pub fn ask() -> Result<View, String> {
    if !cfg!(target_os = "linux") {
        return Err("This runs on the Keel computer.".into());
    }
    let here = house_address().ok_or("Keel could not see the house network.")?;
    let (method, forwarded, refused) = if let Ok(gateway) = igd_next::search_gateway(search_options()) {
        let mut forwarded = Vec::new();
        let mut refused = Vec::new();
        for (proto, port) in PORTS {
            let local = SocketAddr::V4(SocketAddrV4::new(here, *port));
            let kind = match proto {
                Proto::Tcp => igd_next::PortMappingProtocol::TCP,
                Proto::Udp => igd_next::PortMappingProtocol::UDP,
            };
            match gateway.add_port(kind, *port, local, LEASE_SECONDS, DESCRIPTION) {
                Ok(()) => forwarded.push(label(*proto, *port)),
                Err(_) => refused.push(label(*proto, *port)),
            }
        }
        ("UPnP", forwarded, refused)
    } else if let Some(gateway) = default_gateway().filter(|gateway| natpmp_public(*gateway).is_some()) {
        let mut forwarded = Vec::new();
        let mut refused = Vec::new();
        for (proto, port) in PORTS {
            if natpmp_map(gateway, *proto, *port, LEASE_SECONDS) == Some(*port) {
                forwarded.push(label(*proto, *port));
            } else {
                refused.push(label(*proto, *port));
            }
        }
        ("NAT-PMP", forwarded, refused)
    } else {
        return Err("The router did not answer. Its settings may have UPnP turned off. Its settings page can send TCP 80 and 443 and UDP 10000-10019 to this computer by hand.".into());
    };
    let note = if refused.is_empty() {
        format!("The router sends {} to this computer. Keel asks again every 20 minutes.", said(&forwarded))
    } else if forwarded.is_empty() {
        format!("The router answered but would not send {}. Another device may already use those ports.", said(&refused))
    } else {
        format!(
            "The router sends {} here, but would not send {}. Another device may already use those.",
            said(&forwarded),
            said(&refused)
        )
    };
    let saved = Saved {
        asked: !forwarded.is_empty(),
        method: method.into(),
        forwarded,
        note,
    };
    store(&saved);
    Ok(View {
        state: "open",
        asked: saved.asked,
        method: saved.method.clone(),
        forwarded: saved.forwarded.clone(),
        note: saved.note.clone(),
        ..View::default()
    })
}

/// Asks the router again, quietly, while Keel is asking at all.
pub fn renew() {
    if wanted() {
        let _ = ask();
    }
}

/// Takes the ports back from the router and stops asking.
pub fn stop() -> Result<(), String> {
    let saved = load();
    if cfg!(target_os = "linux") {
        if saved.method == "NAT-PMP" {
            if let Some(gateway) = default_gateway() {
                for (proto, port) in PORTS {
                    let _ = natpmp_map(gateway, *proto, *port, 0);
                }
            }
        } else if let Ok(gateway) = igd_next::search_gateway(search_options()) {
            for (proto, port) in PORTS {
                let kind = match proto {
                    Proto::Tcp => igd_next::PortMappingProtocol::TCP,
                    Proto::Udp => igd_next::PortMappingProtocol::UDP,
                };
                let _ = gateway.remove_port(kind, *port);
            }
        }
    }
    store(&Saved {
        asked: false,
        note: "Keel stopped asking the router. The ports close at the router.".into(),
        ..Saved::default()
    });
    Ok(())
}

/// "TCP 80, TCP 443, UDP 10000-10019" instead of twenty-two names.
fn said(labels: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut run: Option<(u16, u16)> = None;
    let flush = |run: &mut Option<(u16, u16)>, out: &mut Vec<String>| {
        if let Some((a, b)) = run.take() {
            out.push(if a == b { format!("UDP {a}") } else { format!("UDP {a}-{b}") });
        }
    };
    for label in labels {
        match label.strip_prefix("UDP ").and_then(|p| p.parse::<u16>().ok()) {
            Some(port) => match run {
                Some((a, b)) if port == b + 1 => run = Some((a, port)),
                _ => {
                    flush(&mut run, &mut out);
                    run = Some((port, port));
                }
            },
            None => {
                flush(&mut run, &mut out);
                out.push(label.clone());
            }
        }
    }
    flush(&mut run, &mut out);
    out.join(", ")
}

fn label(proto: Proto, port: u16) -> String {
    format!("{} {port}", proto.word())
}

/// This computer's address on the house network: the one it would use to
/// reach the internet. No packet is sent.
pub fn house_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    match socket.local_addr().ok()? {
        SocketAddr::V4(addr) if !addr.ip().is_loopback() && !addr.ip().is_unspecified() => Some(*addr.ip()),
        _ => None,
    }
}

/// The router: the default route's gateway.
fn default_gateway() -> Option<Ipv4Addr> {
    gateway_in(&fs::read_to_string("/proc/net/route").ok()?)
}

/// Reads the default gateway from the kernel's route table.
pub fn gateway_in(table: &str) -> Option<Ipv4Addr> {
    for line in table.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 || fields[1] != "00000000" {
            continue;
        }
        let raw = u32::from_str_radix(fields[2], 16).ok()?;
        if raw == 0 {
            continue;
        }
        // The kernel writes the address in the computer's own byte order.
        return Some(Ipv4Addr::from(raw.to_le_bytes()));
    }
    None
}

/// NAT-PMP: ask the router for its outside address.
fn natpmp_public(gateway: Ipv4Addr) -> Option<Ipv4Addr> {
    let answer = natpmp_talk(gateway, &[0, 0])?;
    parse_natpmp_public(&answer)
}

pub fn parse_natpmp_public(answer: &[u8]) -> Option<Ipv4Addr> {
    if answer.len() < 12 || answer[0] != 0 || answer[1] != 128 || answer[2..4] != [0, 0] {
        return None;
    }
    Some(Ipv4Addr::new(answer[8], answer[9], answer[10], answer[11]))
}

/// NAT-PMP: ask the router to send a port here. Lifetime 0 takes it back.
/// The answer is the outside port the router gave, which must be the same.
fn natpmp_map(gateway: Ipv4Addr, proto: Proto, port: u16, lifetime: u32) -> Option<u16> {
    let answer = natpmp_talk(gateway, &natpmp_request(proto, port, lifetime))?;
    parse_natpmp_map(&answer, proto)
}

pub fn natpmp_request(proto: Proto, port: u16, lifetime: u32) -> Vec<u8> {
    let op = match proto {
        Proto::Udp => 1,
        Proto::Tcp => 2,
    };
    let mut request = vec![0, op, 0, 0];
    request.extend(port.to_be_bytes());
    request.extend(port.to_be_bytes());
    request.extend(lifetime.to_be_bytes());
    request
}

pub fn parse_natpmp_map(answer: &[u8], proto: Proto) -> Option<u16> {
    let op = match proto {
        Proto::Udp => 129,
        Proto::Tcp => 130,
    };
    if answer.len() < 16 || answer[0] != 0 || answer[1] != op || answer[2..4] != [0, 0] {
        return None;
    }
    Some(u16::from_be_bytes([answer[10], answer[11]]))
}

fn natpmp_talk(gateway: Ipv4Addr, request: &[u8]) -> Option<Vec<u8>> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.set_read_timeout(Some(Duration::from_millis(800))).ok()?;
    let to = SocketAddrV4::new(gateway, 5351);
    for _ in 0..3 {
        socket.send_to(request, to).ok()?;
        let mut buf = [0u8; 32];
        if let Ok((len, from)) = socket.recv_from(&mut buf) {
            if from.ip() == IpAddr::V4(gateway) {
                return Some(buf[..len].to_vec());
            }
        }
    }
    None
}

/// The address the internet sees for this house, from Cloudflare's address
/// page. Nothing is sent but the request itself.
pub async fn public_address() -> Option<Ipv4Addr> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(6)).build().ok()?;
    for url in ["https://1.1.1.1/cdn-cgi/trace", "https://cloudflare.com/cdn-cgi/trace"] {
        if let Ok(response) = client.get(url).send().await {
            if let Ok(text) = response.text().await {
                if let Some(ip) = address_in_trace(&text) {
                    return Some(ip);
                }
            }
        }
    }
    None
}

pub fn address_in_trace(text: &str) -> Option<Ipv4Addr> {
    text.lines().find_map(|line| line.strip_prefix("ip=")).and_then(|ip| ip.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_ports_are_said_in_few_words() {
        let labels: Vec<String> = super::PORTS.iter().map(|(p, n)| super::label(*p, *n)).collect();
        assert_eq!(super::said(&labels), "TCP 80, TCP 443, UDP 10000-10019");
        assert_eq!(super::said(&["UDP 10000".into(), "UDP 10002".into()]), "UDP 10000, UDP 10002");
    }

    use super::*;

    #[test]
    fn the_two_addresses_say_whether_calls_can_come_in() {
        let public = "102.89.1.2".parse().ok();
        assert_eq!(classify(public, public, "UPnP").0, "open");
        assert_eq!(classify("100.70.3.4".parse().ok(), public, "UPnP").0, "shared");
        assert!(classify("100.70.3.4".parse().ok(), public, "UPnP").1.contains("shares one address"));
        assert_eq!(classify("192.168.1.10".parse().ok(), public, "UPnP").0, "shared");
        assert!(classify("192.168.1.10".parse().ok(), public, "UPnP").1.contains("two routers"));
        assert_eq!(classify("41.1.1.1".parse().ok(), public, "UPnP").0, "shared");
        assert_eq!(classify(None, public, "").0, "unknown");
        assert_eq!(classify(None, None, "").0, "unknown");
    }

    #[test]
    fn the_router_is_read_from_the_route_table() {
        let table = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
                     eth0\t00000000\t0100A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
                     eth0\t0000A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n";
        assert_eq!(gateway_in(table), Some(Ipv4Addr::new(192, 168, 0, 1)));
        assert_eq!(gateway_in("Iface\tDestination\tGateway\n"), None);
    }

    #[test]
    fn nat_pmp_messages_are_built_and_read() {
        assert_eq!(natpmp_request(Proto::Tcp, 443, 3600), vec![0, 2, 0, 0, 1, 187, 1, 187, 0, 0, 14, 16]);
        let public = [0, 128, 0, 0, 0, 0, 0, 9, 102, 89, 1, 2];
        assert_eq!(parse_natpmp_public(&public), Some(Ipv4Addr::new(102, 89, 1, 2)));
        let mapped = [0, 130, 0, 0, 0, 0, 0, 9, 1, 187, 1, 187, 0, 0, 14, 16];
        assert_eq!(parse_natpmp_map(&mapped, Proto::Tcp), Some(443));
        let refused = [0, 130, 0, 2, 0, 0, 0, 9, 1, 187, 0, 0, 0, 0, 0, 0];
        assert_eq!(parse_natpmp_map(&refused, Proto::Tcp), None);
    }

    #[test]
    fn the_public_address_is_read_from_the_trace() {
        let trace = "fl=1\nh=1.1.1.1\nip=102.89.1.2\nts=1\n";
        assert_eq!(address_in_trace(trace), Some(Ipv4Addr::new(102, 89, 1, 2)));
        assert_eq!(address_in_trace("ip=2a00::1\n"), None);
    }
}
