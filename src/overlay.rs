use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;

#[derive(Serialize, serde::Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Headscale,
    Tailscale,
    Unknown,
}

#[derive(Deserialize)]
pub struct EnrollRequest {
    #[serde(default)]
    pub provider: Provider,
    pub control_url: Option<String>,
    pub auth_key: String,
    pub hostname: Option<String>,
}

#[derive(Serialize)]
pub struct EnrollResult {
    pub ok: bool,
    pub provider: Provider,
    pub message: String,
}

impl Default for Provider {
    fn default() -> Self {
        Self::Headscale
    }
}

impl Provider {
    pub fn from_name(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "headscale" => Self::Headscale,
            "tailscale" => Self::Tailscale,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Connected,
    NotConfigured,
    Unavailable,
}

#[derive(Serialize)]
pub struct Status {
    pub provider: Provider,
    pub state: State,
    pub control_url: Option<String>,
    pub overlay_ip: Option<String>,
    pub dns_name: Option<String>,
    pub client_installed: bool,
    pub why: String,
}

pub fn status() -> Status {
    let config = crate::config::load();
    let provider = config.overlay.provider;
    let control_url = config.overlay.headscale_url.clone();
    if !cfg!(target_os = "linux") {
        return Status {
            provider,
            state: State::Unavailable,
            control_url,
            overlay_ip: None,
            dns_name: None,
            client_installed: false,
            why: "Away connects on the Keel computer. This Windows copy is only for trying the screen.".into(),
        };
    }

    let output = Command::new("tailscale")
        .args(["status", "--json"])
        .output();

    match output {
        Ok(output) if output.status.success() => parse_status(provider, &output.stdout),
        Ok(_) => Status {
            provider,
            state: State::NotConfigured,
            control_url,
            overlay_ip: None,
            dns_name: None,
            client_installed: true,
            why: "The Away program is on this computer, but it is not connected yet.".into(),
        },
        Err(_) => Status {
            provider,
            state: State::NotConfigured,
            control_url,
            overlay_ip: None,
            dns_name: None,
            client_installed: false,
            why: "The Away program is not on this computer yet.".into(),
        },
    }
}

const INSTALL_SCRIPT: &str = r#"
set -euo pipefail
say() { echo "$1" >&2; exit 1; }
if command -v tailscale >/dev/null 2>&1; then
  exit 0
fi
if [[ ! -r /etc/os-release ]]; then
  say "Keel could not tell which Debian this computer uses."
fi
# shellcheck disable=SC1091
. /etc/os-release
case "${VERSION_CODENAME:-}" in
  bookworm|trixie) ;;
  *) say "This computer's Debian cannot install Away yet." ;;
esac
if [[ ! -x /usr/bin/curl ]]; then
  say "The download tool is not on this computer."
fi
install -d -m 0755 /usr/share/keyrings /etc/apt/sources.list.d
err=$(mktemp)
fetch() {
  if ! /usr/bin/curl -4 -fsSL --retry 3 --max-time 60 "$1" -o "$2" 2>"$err"; then
    detail=$(tail -n 1 "$err" 2>/dev/null || true)
    say "Keel could not download the Away program for ${VERSION_CODENAME}. ${detail:-Check the network and try again.}"
  fi
  chmod 0644 "$2"
}
fetch "https://pkgs.tailscale.com/stable/debian/${VERSION_CODENAME}.noarmor.gpg" /usr/share/keyrings/tailscale-archive-keyring.gpg
fetch "https://pkgs.tailscale.com/stable/debian/${VERSION_CODENAME}.tailscale-keyring.list" /etc/apt/sources.list.d/tailscale.list
log=$(mktemp)
if ! apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1; then
  detail=$(grep -E 'Err:|^E:' "$log" | tail -n 1 || true)
  say "Keel could not download the Away program. ${detail:-Check the network and try again.}"
fi
if ! DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true tailscale >"$log" 2>&1; then
  say "Keel could not install the Away program. Try again."
fi
if ! systemctl enable --now tailscaled >"$log" 2>&1; then
  say "The Away program installed, but it did not start."
fi
rm -f "$log"
"#;

pub fn install_client() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("Away installs on the Keel computer.".into());
    }
    // The script goes on stdin. systemd-run expands ${...} on the command
    // line, which would erase the Debian codename and request a missing file.
    crate::storage::host("exec /bin/bash -s", &[], Some(INSTALL_SCRIPT))?;
    Ok("The Away program is ready. Paste the one-time key and press Connect.".into())
}

pub fn enroll(req: EnrollRequest) -> EnrollResult {
    if !cfg!(target_os = "linux") {
        return failed(
            req.provider,
            "Overlay enrollment is available on the Linux Keel appliance only.",
        );
    }
    if req.auth_key.trim().is_empty() {
        return failed(req.provider, "an enrollment key is required");
    }

    let control_url = match req.provider {
        Provider::Headscale => match req.control_url.as_deref().and_then(valid_control_url) {
            Some(url) => Some(url.to_string()),
            None => return failed(req.provider, "Type the address of the computer you run. It has to start with https."),
        },
        Provider::Tailscale => None,
        Provider::Unknown => return failed(req.provider, "unknown overlay provider"),
    };
    let hostname = req.hostname.as_deref().unwrap_or("keel").trim().to_string();
    if !valid_hostname(&hostname) {
        return failed(
            req.provider,
            "hostname must contain only letters, numbers, dots, and dashes",
        );
    }

    let mut args = vec!["up".to_string(), "--auth-key".to_string(), req.auth_key];
    if let Some(url) = control_url.as_deref() {
        args.extend(["--login-server".into(), url.into()]);
    }
    args.extend(["--hostname".into(), hostname]);

    let result = Command::new("tailscale").args(&args).output();
    match result {
        Ok(output) if output.status.success() => {
            let mut config = crate::config::load();
            config.overlay.provider = req.provider;
            if req.provider == Provider::Headscale {
                config.overlay.headscale_url = control_url;
            }
            match crate::config::save(&config) {
                Ok(()) => EnrollResult {
                    ok: true,
                    provider: req.provider,
                    message: "Overlay enrollment succeeded. Keel saved the provider URL, not the enrollment key.".into(),
                },
                Err(_) => EnrollResult {
                    ok: true,
                    provider: req.provider,
                    message: "Overlay enrollment succeeded, but Keel could not save its provider configuration.".into(),
                },
            }
        }
        Ok(_) => failed(
            req.provider,
            "the overlay client refused enrollment; check the URL and enrollment key",
        ),
        Err(_) => failed(
            req.provider,
            "Install the Away program first.",
        ),
    }
}

fn failed(provider: Provider, message: &str) -> EnrollResult {
    EnrollResult {
        ok: false,
        provider,
        message: message.into(),
    }
}

fn valid_control_url(value: &str) -> Option<&str> {
    let value = value.trim().trim_end_matches('/');
    (value.starts_with("https://")
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\''))
    .then_some(value)
}

fn valid_hostname(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= hostname_limit()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

const fn hostname_limit() -> usize {
    253
}

fn parse_status(provider: Provider, raw: &[u8]) -> Status {
    let value: Value = match serde_json::from_slice(raw) {
        Ok(value) => value,
        Err(_) => {
            return Status {
                provider,
                state: State::Unavailable,
                control_url: None,
                overlay_ip: None,
                dns_name: None,
                client_installed: true,
                why: "The Away program answered, but Keel could not read it.".into(),
            }
        }
    };

    let running = value
        .get("BackendState")
        .and_then(Value::as_str)
        .map(|state| state.eq_ignore_ascii_case("running"))
        .unwrap_or(false);
    let overlay_ip = value
        .get("Self")
        .and_then(|self_node| self_node.get("TailscaleIPs"))
        .and_then(Value::as_array)
        .and_then(|ips| ips.iter().find_map(Value::as_str))
        .map(str::to_string);
    let dns_name = value
        .get("Self")
        .and_then(|self_node| self_node.get("DNSName"))
        .and_then(Value::as_str)
        .map(|name| name.trim_end_matches('.').to_string())
        .filter(|name| !name.is_empty());

    if running && overlay_ip.is_some() {
        Status {
            provider,
            state: State::Connected,
            control_url: None,
            overlay_ip,
            dns_name,
            client_installed: true,
            why: "This computer is connected. The same folders open from a phone or laptop that is connected too.".into(),
        }
    } else {
        Status {
            provider,
            state: State::NotConfigured,
            control_url: None,
            overlay_ip,
            dns_name,
            client_installed: true,
            why: "The Away program is on this computer, but it is not connected yet.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_status, valid_control_url, valid_hostname, Provider, State};

    #[test]
    fn provider_names_are_explicit() {
        assert!(matches!(
            Provider::from_name("headscale"),
            Provider::Headscale
        ));
        assert!(matches!(
            Provider::from_name("tailscale"),
            Provider::Tailscale
        ));
        assert!(matches!(Provider::from_name("other"), Provider::Unknown));
    }

    #[test]
    fn parses_connected_client_status() {
        let status = parse_status(
            Provider::Headscale,
            br#"{"BackendState":"Running","Self":{"TailscaleIPs":["100.64.0.2"],"DNSName":"keel.example.ts.net."}}"#,
        );
        assert!(matches!(status.state, State::Connected));
        assert_eq!(status.overlay_ip.as_deref(), Some("100.64.0.2"));
        assert_eq!(status.dns_name.as_deref(), Some("keel.example.ts.net"));
    }

    #[test]
    fn validates_headscale_url_and_hostname() {
        assert_eq!(
            valid_control_url("https://hs.example/"),
            Some("https://hs.example")
        );
        assert_eq!(valid_control_url("http://hs.example"), None);
        assert!(valid_hostname("keel-nas.example"));
        assert!(!valid_hostname("keel nas"));
    }

    #[test]
    fn away_install_uses_the_tailscale_package_and_keeps_the_key_out() {
        let script = super::INSTALL_SCRIPT;
        assert!(script.contains("pkgs.tailscale.com/stable/debian/${VERSION_CODENAME}.noarmor.gpg"));
        assert!(script.contains("bookworm|trixie"));
        assert!(script.contains("Acquire::ForceIPv4=true"));
        assert!(script.contains("chmod 0644"));
        assert!(!script.contains("auth-key"));
        assert!(!script.contains("tskey-"));
    }
}
