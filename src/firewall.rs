//! The page and the folders answer on the house network and on Away.
//!
//! Tailscale's own port stays open, so a wrong house rule does not cut off the
//! phone. The Keel screen stays on loopback. Nothing else may connect.

use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

const FLAG: &str = "firewall.on";

pub fn wanted() -> bool {
    flag_path().is_file()
}

pub fn enable(addresses: &[String]) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("This applies on the Keel computer.".into());
    }
    let subnets = house_subnets(addresses);
    let rules = ruleset(&subnets)?;
    let script = format!(
        r#"set -uo pipefail
fail() {{ echo "$1" >&2; exit 1; }}
if ! command -v nft >/dev/null 2>&1; then
  apt-get update -o Acquire::ForceIPv4=true >/dev/null 2>&1 || true
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true nftables >/dev/null 2>&1 || fail "Keel could not install the firewall."
fi
nft delete table inet keel >/dev/null 2>&1 || true
nft -f - <<'EOF'
{rules}
EOF
"#
    );
    crate::storage::host("exec /bin/bash -s", &[], Some(&script))?;
    remember()?;
    let list = subnets.join(", ");
    Ok(format!(
        "The page and the folders answer on {list} and on Away. The Keel screen still works. Other computers are refused."
    ))
}

pub fn house_subnets(addresses: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for address in addresses {
        let Ok(ip) = address.parse::<Ipv4Addr>() else {
            continue;
        };
        if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || cgnat(ip) {
            continue;
        }
        let [a, b, c, _] = ip.octets();
        let subnet = format!("{a}.{b}.{c}.0/24");
        if !found.contains(&subnet) {
            found.push(subnet);
        }
    }
    found
}

pub fn ruleset(subnets: &[String]) -> Result<String, String> {
    if subnets.is_empty() {
        return Err("Keel could not see the house network, so it did not change the firewall.".into());
    }
    for subnet in subnets {
        if !safe_subnet(subnet) {
            return Err("Keel could not see the house network, so it did not change the firewall.".into());
        }
    }
    let mut house = String::new();
    for subnet in subnets {
        house.push_str(&format!(
            "    ip saddr {subnet} tcp dport {{ 445, 9478 }} accept\n"
        ));
    }
    Ok(format!(
        r#"table inet keel {{
  chain input {{
    type filter hook input priority 0; policy drop;
    iif "lo" accept
    ct state established,related accept
    ct state invalid drop
    icmp type echo-request accept
    icmpv6 type {{ echo-request, nd-neighbor-solicit, nd-neighbor-advert, nd-router-advert }} accept
    udp sport 67 udp dport 68 accept
    udp dport 41641 accept
    iifname "tailscale0" tcp dport {{ 445, 9478, 443 }} accept
{house}  }}
}}
"#
    ))
}

fn safe_subnet(subnet: &str) -> bool {
    let Some((addr, prefix)) = subnet.split_once('/') else {
        return false;
    };
    prefix == "24" && addr.parse::<Ipv4Addr>().is_ok() && addr.ends_with(".0")
}

fn cgnat(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 100 && (64..128).contains(&b)
}

fn flag_path() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join(FLAG)
}

fn remember() -> Result<(), String> {
    let path = flag_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "Keel applied the firewall but could not remember it.".to_string())?;
    }
    fs::write(&path, b"on\n").map_err(|_| "Keel applied the firewall but could not remember it.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_house_subnet_skips_away_and_the_screen() {
        let subnets = house_subnets(&[
            "192.168.0.191".into(),
            "127.0.0.1".into(),
            "100.113.90.33".into(),
            "169.254.1.1".into(),
        ]);
        assert_eq!(subnets, vec!["192.168.0.0/24".to_string()]);
    }

    #[test]
    fn the_rules_allow_the_house_and_away_and_drop_the_rest() {
        let rules = ruleset(&["192.168.0.0/24".into()]).unwrap();
        assert!(rules.contains("policy drop"));
        assert!(rules.contains("iif \"lo\" accept"));
        assert!(rules.contains("udp dport 41641 accept"));
        assert!(rules.contains("iifname \"tailscale0\" tcp dport { 445, 9478, 443 } accept"));
        assert!(rules.contains("ip saddr 192.168.0.0/24 tcp dport { 445, 9478 } accept"));
        assert!(!rules.contains("dport 22"));
        assert!(ruleset(&[]).is_err());
        assert!(ruleset(&["10.0.0.1/32".into()]).is_err());
    }
}
