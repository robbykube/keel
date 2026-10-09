use axum::extract::{ConnectInfo, Request};
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub token: String,
}

/// Physical-console trust. The person sitting at the appliance cannot read the
/// admin token: reading it needs a terminal, and the terminal is the thing this
/// appliance must never show them. So the local kiosk is trusted by position —
/// the same trust the hardware already grants, since anyone at the keyboard can
/// power-cycle the box and boot it single-user.
///
/// This trusts every process on the box, not only the kiosk, so it stays off
/// unless the kiosk drop-in turns it on. `memory-bank/os.md` records the unix
/// socket that narrows it back to the kiosk alone.
///
/// Never enable this behind a reverse proxy. A proxy on the same host makes
/// every forwarded request look like it came from loopback, which turns this
/// into an unauthenticated admin API for the whole network. keel deliberately
/// ignores `X-Forwarded-For` rather than trusting a header an attacker can set.
pub fn loopback_trusted() -> bool {
    std::env::var("KEEL_TRUST_LOOPBACK")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

fn from_loopback(request: &Request) -> bool {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().is_loopback())
        .unwrap_or(false)
}

pub async fn require_admin(request: Request, next: Next) -> Response {
    let expected = match std::env::var("KEEL_ADMIN_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token,
        _ => return next.run(request).await,
    };

    if loopback_trusted() && from_loopback(&request) {
        return next.run(request).await;
    }

    let authorized = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| token == expected)
        .unwrap_or(false)
        || request
            .headers()
            .get(COOKIE)
            .and_then(|value| value.to_str().ok())
            .map(|cookies| {
                cookies.split(';').any(|cookie| {
                    cookie.trim().strip_prefix("keel_session=") == Some(expected.as_str())
                })
            })
            .unwrap_or(false);

    if authorized {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, "admin authentication required").into_response()
    }
}

pub fn verify(token: &str) -> bool {
    std::env::var("KEEL_ADMIN_TOKEN")
        .map(|expected| !expected.trim().is_empty() && token == expected)
        .unwrap_or(false)
}

pub fn token_configured() -> bool {
    std::env::var("KEEL_ADMIN_TOKEN")
        .map(|token| !token.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{loopback_trusted, token_configured};

    #[test]
    fn empty_token_is_not_configured() {
        std::env::set_var("KEEL_ADMIN_TOKEN", "  ");
        assert!(!token_configured());
        std::env::remove_var("KEEL_ADMIN_TOKEN");
    }

    // One test, not several: these share process-wide environment, and separate
    // tests would race each other under the parallel test runner.
    #[test]
    fn loopback_trust_is_off_until_explicitly_set_to_one() {
        std::env::remove_var("KEEL_TRUST_LOOPBACK");
        assert!(!loopback_trusted());
        std::env::set_var("KEEL_TRUST_LOOPBACK", "0");
        assert!(!loopback_trusted());
        std::env::set_var("KEEL_TRUST_LOOPBACK", "yes");
        assert!(!loopback_trusted());
        std::env::set_var("KEEL_TRUST_LOOPBACK", "1");
        assert!(loopback_trusted());
        std::env::remove_var("KEEL_TRUST_LOOPBACK");
    }
}
