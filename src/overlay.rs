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
            why: "Away connects on the Keel computer. This Windows copy is only for trying the screen.".into(),
        };
    }

    let output = Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .ok();

    match output {
        Some(output) if output.status.success() => parse_status(provider, &output.stdout),
        Some(_) | None => Status {
            provider,
            state: State::NotConfigured,
            control_url,
            overlay_ip: None,
            dns_name: None,
            why: "Install and enroll the overlay client. Headscale is the self-hosted-first control plane; Tailscale clients can use either provider.".into(),
        },
    }
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
            "the overlay client is not installed on this Linux host",
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
                why: "The overlay client answered, but its status was not readable.".into(),
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
            why: "The overlay client reports a connected node. Keel still tests file and GUI paths separately.".into(),
        }
    } else {
        Status {
            provider,
            state: State::NotConfigured,
            control_url: None,
            overlay_ip,
            dns_name,
            why: "The overlay client is installed but not connected to a control plane.".into(),
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
}
