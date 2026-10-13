use crate::updates::{github_client_id, github_token_is_set, save_github_token};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

struct Pending {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Duration,
    expires_at: Instant,
    last_poll: Option<Instant>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct GithubConnectStatus {
    pub state: String,
    pub user_code: Option<String>,
    pub verification_uri: Option<String>,
    pub message: String,
    pub github_token_set: bool,
    pub can_connect: bool,
    pub needs_setup: bool,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    interval: Option<u64>,
}

fn pending() -> &'static Mutex<Option<Pending>> {
    static PENDING: OnceLock<Mutex<Option<Pending>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(None))
}

pub async fn start() -> Result<GithubConnectStatus, String> {
    let client_id = github_client_id();
    if client_id.is_empty() {
        return Err("Keel cannot start GitHub sign-in from this image yet. Paste a token below.".into());
    }
    {
        let Ok(guard) = pending().lock() else {
            return Err("GitHub sign-in is busy. Try again.".into());
        };
        if let Some(current) = guard.as_ref() {
            if Instant::now() < current.expires_at {
                return Ok(pending_status(current));
            }
        }
    }
    let body = if client_id.starts_with("Iv") {
        form(&[("client_id", &client_id)])
    } else {
        form(&[("client_id", &client_id), ("scope", "repo")])
    };
    let payload = github_form_post(DEVICE_CODE_URL, body).await?;
    let parsed: DeviceCodeResponse = serde_json::from_str(&payload)
        .map_err(|_| "GitHub sent an unexpected sign-in reply.".to_string())?;
    if let Some(error) = parsed.error.as_deref() {
        return Err(connect_error(error, parsed.error_description.as_deref()));
    }
    if parsed.device_code.is_empty() || parsed.user_code.is_empty() {
        return Err("GitHub did not send a sign-in code.".into());
    }
    let interval = Duration::from_secs(parsed.interval.unwrap_or(5).max(5));
    let next = Pending {
        device_code: parsed.device_code,
        user_code: parsed.user_code,
        verification_uri: verification_uri(&parsed.verification_uri),
        interval,
        expires_at: Instant::now() + Duration::from_secs(parsed.expires_in.max(30)),
        last_poll: None,
    };
    let status = pending_status(&next);
    let Ok(mut guard) = pending().lock() else {
        return Err("GitHub sign-in is busy. Try again.".into());
    };
    *guard = Some(next);
    Ok(status)
}

pub async fn poll() -> Result<GithubConnectStatus, String> {
    let client_id = github_client_id();
    let snapshot = {
        let Ok(mut guard) = pending().lock() else {
            return Err("GitHub sign-in is busy. Try again.".into());
        };
        match guard.as_mut() {
            None => return Ok(status_without_lock()),
            Some(current) if Instant::now() >= current.expires_at => {
                *guard = None;
                return Ok(GithubConnectStatus {
                    state: "expired".into(),
                    user_code: None,
                    verification_uri: None,
                    message: "That GitHub code expired. Press Connect GitHub to get a new one."
                        .into(),
                    github_token_set: github_token_is_set(),
                    can_connect: crate::updates::github_can_connect(),
                    needs_setup: false,
                });
            }
            Some(current) => {
                if current
                    .last_poll
                    .is_some_and(|last| last.elapsed() < current.interval)
                {
                    return Ok(pending_status(current));
                }
                current.last_poll = Some(Instant::now());
                (
                    current.device_code.clone(),
                    current.interval,
                    pending_status(current),
                )
            }
        }
    };
    let (device_code, interval, waiting) = snapshot;
    let body = form(&[
        ("client_id", &client_id),
        ("device_code", &device_code),
        ("grant_type", GRANT_TYPE),
    ]);
    let payload = github_form_post(ACCESS_TOKEN_URL, body).await?;
    let parsed: AccessTokenResponse = serde_json::from_str(&payload)
        .map_err(|_| "GitHub sent an unexpected sign-in reply.".to_string())?;
    if let Some(token) = parsed.access_token.as_deref() {
        let message = save_github_token(token)?;
        if let Ok(mut guard) = pending().lock() {
            *guard = None;
        }
        return Ok(GithubConnectStatus {
            state: "connected".into(),
            user_code: None,
            verification_uri: None,
            message,
            github_token_set: true,
            can_connect: crate::updates::github_can_connect(),
            needs_setup: false,
        });
    }
    match parsed.error.as_deref() {
        None | Some("authorization_pending") => Ok(waiting),
        Some("slow_down") => {
            if let Ok(mut guard) = pending().lock() {
                if let Some(current) = guard.as_mut() {
                    let extra = parsed.interval.unwrap_or(5).max(5);
                    current.interval = interval + Duration::from_secs(extra);
                    return Ok(pending_status(current));
                }
            }
            Ok(waiting)
        }
        Some("expired_token") => {
            if let Ok(mut guard) = pending().lock() {
                *guard = None;
            }
            Ok(GithubConnectStatus {
                state: "expired".into(),
                user_code: None,
                verification_uri: None,
                message: "That GitHub code expired. Press Connect GitHub to get a new one.".into(),
                github_token_set: github_token_is_set(),
                can_connect: crate::updates::github_can_connect(),
                needs_setup: false,
            })
        }
        Some("access_denied") => {
            if let Ok(mut guard) = pending().lock() {
                *guard = None;
            }
            Ok(GithubConnectStatus {
                state: "denied".into(),
                user_code: None,
                verification_uri: None,
                message: "GitHub sign-in was cancelled. Press Connect GitHub to try again.".into(),
                github_token_set: github_token_is_set(),
                can_connect: crate::updates::github_can_connect(),
                needs_setup: false,
            })
        }
        Some(other) => Err(connect_error(other, parsed.error_description.as_deref())),
    }
}

pub fn cancel() -> GithubConnectStatus {
    if let Ok(mut guard) = pending().lock() {
        *guard = None;
    }
    idle_status()
}

fn status_without_lock() -> GithubConnectStatus {
    if github_token_is_set() {
        GithubConnectStatus {
            state: "connected".into(),
            user_code: None,
            verification_uri: None,
            message: "GitHub is connected on this computer.".into(),
            github_token_set: true,
            can_connect: crate::updates::github_can_connect(),
            needs_setup: false,
        }
    } else {
        idle_status()
    }
}

fn idle_status() -> GithubConnectStatus {
    GithubConnectStatus {
        state: "idle".into(),
        user_code: None,
        verification_uri: None,
        message: if crate::updates::github_can_connect() {
            "Press Connect GitHub. A code will appear. Enter it on GitHub from your phone or this PC.".into()
        } else {
            "Press Connect GitHub. GitHub will ask you to allow this computer to read your existing robbykube/keel repository.".into()
        },
        github_token_set: github_token_is_set(),
        can_connect: crate::updates::github_can_connect(),
        needs_setup: !crate::updates::github_can_connect(),
    }
}

fn pending_status(current: &Pending) -> GithubConnectStatus {
    GithubConnectStatus {
        state: "pending".into(),
        user_code: Some(current.user_code.clone()),
        verification_uri: Some(current.verification_uri.clone()),
        message: "On your phone or another computer, open github.com/login/device and enter this code. Then allow Keel to read releases.".into(),
        github_token_set: github_token_is_set(),
        can_connect: true,
        needs_setup: false,
    }
}

pub async fn finish_app(code: &str) -> Result<String, String> {
    let code = code.trim();
    if code.is_empty()
        || code
            .chars()
            .any(|character| !character.is_ascii_alphanumeric())
    {
        return Err("GitHub sent an unexpected setup code.".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(format!("keel/{}", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(format!(
            "https://api.github.com/app-manifests/{code}/conversions"
        ))
        .header(
            reqwest::header::ACCEPT,
            "application/vnd.github+json",
        )
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|error| format!("GitHub could not be reached: {error}"))?;
    if response.url().host_str() != Some("api.github.com") {
        return Err("GitHub sent an unexpected address.".into());
    }
    if !response.status().is_success() {
        return Err("GitHub did not finish allowing this computer.".into());
    }
    let parsed: ManifestConversion = response
        .json()
        .await
        .map_err(|_| "GitHub sent an unexpected setup reply.".to_string())?;
    crate::updates::save_github_client_id(&parsed.client_id)
}

#[derive(Deserialize)]
struct ManifestConversion {
    client_id: String,
}

fn verification_uri(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed == "https://github.com/login/device" || trimmed == "https://github.com/login/device/"
    {
        trimmed.trim_end_matches('/').to_string()
    } else {
        "https://github.com/login/device".into()
    }
}

fn connect_error(error: &str, description: Option<&str>) -> String {
    match error {
        "incorrect_client_credentials" => {
            "GitHub did not recognise this Keel image. Paste a token below, or set a GitHub app id in Settings.".into()
        }
        "unsupported_grant_type" => {
            "This GitHub app cannot use device sign-in. Enable Device Flow on the Keel GitHub App, then try again.".into()
        }
        other => description
            .filter(|text| !text.is_empty())
            .map(|text| text.to_string())
            .unwrap_or_else(|| format!("GitHub sign-in stopped ({other}).")),
    }
}

async fn github_form_post(url: &str, body: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(format!("keel/{}", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|error| format!("GitHub could not be reached: {error}"))?;
    if response.url().host_str() != Some("github.com") {
        return Err("GitHub sent an unexpected address.".into());
    }
    response
        .text()
        .await
        .map_err(|_| "GitHub sent an empty sign-in reply.".into())
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{key}={}", url_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn url_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_code_json_round_trip() {
        let raw = r#"{"device_code":"abc","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#;
        let parsed: DeviceCodeResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.user_code, "WDJB-MJHT");
        assert!(parsed.error.is_none());
    }

    #[test]
    fn pending_status_does_not_expose_device_code() {
        let status = pending_status(&Pending {
            device_code: "secret-device-code".into(),
            user_code: "WDJB-MJHT".into(),
            verification_uri: "https://github.com/login/device".into(),
            interval: Duration::from_secs(5),
            expires_at: Instant::now() + Duration::from_secs(60),
            last_poll: None,
        });
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("WDJB-MJHT"));
        assert!(!json.contains("secret-device-code"));
        assert!(!json.contains("device_code"));
    }

    #[test]
    fn form_encodes_grant_type() {
        let body = form(&[("grant_type", GRANT_TYPE)]);
        assert!(body.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
    }

    #[test]
    fn manifest_conversion_reads_client_id() {
        let parsed: ManifestConversion =
            serde_json::from_str(r#"{"client_id":"Iv1.keeltestid","client_secret":"do-not-keep"}"#)
                .unwrap();
        assert_eq!(parsed.client_id, "Iv1.keeltestid");
    }

    #[test]
    fn only_github_device_uri_is_shown() {
        assert_eq!(
            verification_uri("https://evil.example/login"),
            "https://github.com/login/device"
        );
        assert_eq!(
            verification_uri("https://github.com/login/device/"),
            "https://github.com/login/device"
        );
    }
}
