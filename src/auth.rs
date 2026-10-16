use axum::extract::{ConnectInfo, Path, Request};
use axum::http::header::{AUTHORIZATION, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
pub struct LoginRequest {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Deserialize)]
pub struct PersonBody {
    pub name: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Deserialize)]
pub struct DecideBody {
    pub allow: bool,
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

fn session_from(headers: &HeaderMap) -> Option<String> {
    headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|cookie| cookie.trim().strip_prefix("keel_session="))
                .map(str::to_string)
        })
}

fn is_authorized(request: &Request) -> bool {
    let expected = match std::env::var("KEEL_ADMIN_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token,
        _ => return true,
    };
    if loopback_trusted() && from_loopback(request) {
        return true;
    }
    request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| token == expected)
        .unwrap_or(false)
        || session_from(request.headers()).is_some_and(|value| credential_ok(&value, &expected))
}

pub async fn require_admin(request: Request, next: Next) -> Response {
    if is_authorized(&request) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            "Sign in first. Ask the Keel computer, or open Sign in and use the admin token.",
        )
            .into_response()
    }
}

pub fn verify(token: &str) -> bool {
    std::env::var("KEEL_ADMIN_TOKEN")
        .map(|expected| !expected.trim().is_empty() && token == expected)
        .unwrap_or(false)
}

/// The appliance screen is the only place that may Allow a sign-in. A browser
/// on another computer can ask, and the person at the Keel computer presses
/// the button. That keeps the admin token off the everyday path.
pub fn screen_may_answer(loopback: bool) -> bool {
    loopback
}

const PENDING_TTL: Duration = Duration::from_secs(120);
const ANSWER_TTL: Duration = Duration::from_secs(60);
const SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_WAITING: usize = 4;

#[derive(Clone)]
enum RequestState {
    Waiting,
    Allowed(String),
    Denied,
}

struct Pending {
    id: String,
    from: IpAddr,
    state: RequestState,
    keep_until: Instant,
}

struct Live {
    expires: Instant,
    who: String,
}

struct Gate {
    requests: Vec<Pending>,
    sessions: HashMap<String, Live>,
}

impl Gate {
    fn purge(&mut self, now: Instant) {
        self.requests.retain(|request| request.keep_until > now);
        self.sessions.retain(|_, live| live.expires > now);
    }
}

fn gate() -> std::sync::MutexGuard<'static, Gate> {
    static GATE: OnceLock<Mutex<Gate>> = OnceLock::new();
    GATE.get_or_init(|| {
        Mutex::new(Gate {
            requests: Vec::new(),
            sessions: HashMap::new(),
        })
    })
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn random_id() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "could not start a sign-in request".to_string())?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut id = String::with_capacity(64);
    for byte in bytes {
        id.push(HEX[(byte >> 4) as usize] as char);
        id.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(id)
}

fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn session_cookie(session: &str) -> String {
    format!("keel_session={session}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400")
}

pub fn issue_session(who: &str) -> Result<String, String> {
    let id = random_id()?;
    let mut gate = gate();
    let now = Instant::now();
    gate.purge(now);
    gate.sessions.insert(
        id.clone(),
        Live {
            expires: now + SESSION_TTL,
            who: who.to_string(),
        },
    );
    Ok(id)
}

fn session_who(presented: &str) -> Option<String> {
    let mut gate = gate();
    let now = Instant::now();
    gate.purge(now);
    gate.sessions.get(presented).map(|live| live.who.clone())
}

pub fn credential_ok(presented: &str, admin_token: &str) -> bool {
    if !presented.is_empty() && presented == admin_token {
        return true;
    }
    session_who(presented).is_some()
}

/// Ask the Keel computer to allow this browser. The same address keeps one
/// open request, so a second click does not stack banners.
pub fn create_request(from: IpAddr) -> Result<String, String> {
    let mut gate = gate();
    let now = Instant::now();
    gate.purge(now);
    if let Some(existing) = gate
        .requests
        .iter()
        .find(|request| request.from == from && matches!(request.state, RequestState::Waiting))
    {
        return Ok(existing.id.clone());
    }
    let waiting = gate
        .requests
        .iter()
        .filter(|request| matches!(request.state, RequestState::Waiting))
        .count();
    if waiting >= MAX_WAITING {
        return Err(
            "Someone else is already waiting at the Keel computer. Try again in a minute.".into(),
        );
    }
    let id = random_id()?;
    gate.requests.push(Pending {
        id: id.clone(),
        from,
        state: RequestState::Waiting,
        keep_until: now + PENDING_TTL,
    });
    Ok(id)
}

#[derive(Serialize)]
pub struct ScreenRequest {
    pub id: String,
    pub message: String,
}

fn screen_message(ip: IpAddr) -> String {
    if ip.is_loopback() {
        "A browser on this computer wants to sign in. Allow it if that was you.".into()
    } else {
        format!("A browser at {ip} wants to sign in. Allow it if that was you.")
    }
}

pub fn pending_for_screen() -> Vec<ScreenRequest> {
    let mut gate = gate();
    let now = Instant::now();
    gate.purge(now);
    gate.requests
        .iter()
        .filter(|request| matches!(request.state, RequestState::Waiting))
        .map(|request| ScreenRequest {
            id: request.id.clone(),
            message: screen_message(request.from),
        })
        .collect()
}

pub fn decide(id: &str, allow: bool) -> Result<(), String> {
    if !valid_id(id) {
        return Err("That request has expired. Ask again from the other computer.".into());
    }
    let mut gate = gate();
    let now = Instant::now();
    gate.purge(now);
    let Some(index) = gate.requests.iter().position(|request| request.id == id) else {
        return Err("That request has expired. Ask again from the other computer.".into());
    };
    if !matches!(gate.requests[index].state, RequestState::Waiting) {
        return Err("That request was already answered.".into());
    }
    if allow {
        let session = random_id()?;
        gate.sessions.insert(
            session.clone(),
            Live {
                expires: now + SESSION_TTL,
                who: String::new(),
            },
        );
        gate.requests[index].state = RequestState::Allowed(session);
    } else {
        gate.requests[index].state = RequestState::Denied;
    }
    gate.requests[index].keep_until = now + ANSWER_TTL;
    Ok(())
}

pub enum RequestView {
    Waiting,
    Allowed(String),
    Denied,
    Expired,
}

pub fn poll(id: &str) -> RequestView {
    if !valid_id(id) {
        return RequestView::Expired;
    }
    let mut gate = gate();
    let now = Instant::now();
    gate.purge(now);
    match gate.requests.iter().find(|request| request.id == id) {
        None => RequestView::Expired,
        Some(request) => match &request.state {
            RequestState::Waiting => RequestView::Waiting,
            RequestState::Allowed(session) => RequestView::Allowed(session.clone()),
            RequestState::Denied => RequestView::Denied,
        },
    }
}

fn start_session(who: &str, visit: &str) -> Result<(HeaderMap, Json<serde_json::Value>), (StatusCode, String)> {
    let session = issue_session(who).map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    crate::people::remember(visit);
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&session_cookie(&session))
            .map_err(|_| (StatusCode::BAD_REQUEST, "could not start the sign-in session".into()))?,
    );
    let message = if who.is_empty() {
        "Signed in.".to_string()
    } else {
        format!("Signed in as {who}.")
    };
    Ok((headers, Json(serde_json::json!({ "ok": true, "message": message, "who": who }))))
}

pub async fn login(
    Json(req): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<serde_json::Value>), (StatusCode, String)> {
    if !req.name.trim().is_empty() {
        let who = crate::people::sign_in(&req.name, &req.password)
            .map_err(|error| (StatusCode::UNAUTHORIZED, error))?;
        return start_session(&who, &who);
    }
    if !token_configured() {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            "Add a person in Settings on the Keel screen, then sign in with that name and password.".into(),
        ));
    }
    let token = req.token.trim();
    if !verify(token) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "That admin token does not match. Sign in with a name and password, or type the token again."
                .into(),
        ));
    }
    start_session("", "Admin token")
}

pub async fn create_sign_in(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !token_configured() {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            "Admin sign-in is not set up on this computer yet.".into(),
        ));
    }
    let id = create_request(addr.ip()).map_err(|error| (StatusCode::CONFLICT, error))?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn sign_in_requests(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<Vec<ScreenRequest>>, (StatusCode, String)> {
    if !screen_may_answer(addr.ip().is_loopback()) {
        return Err((
            StatusCode::FORBIDDEN,
            "Sign-in requests show on the Keel computer's own screen.".into(),
        ));
    }
    Ok(Json(pending_for_screen()))
}

pub async fn sign_in_status(
    Path(id): Path<String>,
) -> Result<(HeaderMap, Json<serde_json::Value>), (StatusCode, String)> {
    let (state, session) = match poll(&id) {
        RequestView::Waiting => ("waiting", None),
        RequestView::Allowed(session) => ("allowed", Some(session)),
        RequestView::Denied => ("denied", None),
        RequestView::Expired => ("expired", None),
    };
    let mut headers = HeaderMap::new();
    if let Some(session) = session {
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&session_cookie(&session)).map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "could not start the sign-in session".to_string(),
                )
            })?,
        );
    }
    Ok((headers, Json(serde_json::json!({ "state": state }))))
}

pub async fn answer_sign_in(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    Json(body): Json<DecideBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !screen_may_answer(addr.ip().is_loopback()) {
        return Err((
            StatusCode::FORBIDDEN,
            "Press Allow on the Keel computer's own screen.".into(),
        ));
    }
    decide(&id, body.allow).map_err(|error| (StatusCode::CONFLICT, error))?;
    if body.allow {
        crate::people::remember("Allowed at the screen");
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// What the sidebar shows. The Keel screen itself is always signed in; a
/// browser elsewhere is signed in only while its session cookie is valid.
pub async fn session_state(request: Request) -> Json<serde_json::Value> {
    let signed_in = is_authorized(&request);
    let who = session_from(request.headers())
        .and_then(|session| session_who(&session))
        .unwrap_or_default();
    let can_sign_out = session_from(request.headers()).is_some_and(|session| session_who(&session).is_some());
    Json(serde_json::json!({ "signed_in": signed_in, "can_sign_out": can_sign_out, "who": who }))
}

pub async fn people_view() -> Json<crate::people::PeopleView> {
    Json(crate::people::view())
}

pub async fn people_add(
    Json(body): Json<PersonBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let message = crate::people::add_person(&body.name, &body.password)
        .map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    Ok(Json(serde_json::json!({ "ok": true, "message": message })))
}

pub async fn people_remove(
    Json(body): Json<PersonBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let message = crate::people::remove_person(&body.name)
        .map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    Ok(Json(serde_json::json!({ "ok": true, "message": message })))
}

pub async fn sign_out(headers: HeaderMap) -> (HeaderMap, Json<serde_json::Value>) {
    if let Some(session) = session_from(&headers) {
        gate().sessions.remove(&session);
    }
    let mut reply = HeaderMap::new();
    reply.insert(
        SET_COOKIE,
        HeaderValue::from_static("keel_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
    );
    (reply, Json(serde_json::json!({ "ok": true })))
}

pub fn token_configured() -> bool {
    std::env::var("KEEL_ADMIN_TOKEN")
        .map(|token| !token.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_keel_screen_answers_a_sign_in_once() {
        {
            let mut gate = gate();
            gate.requests.clear();
            gate.sessions.clear();
        }

        assert!(screen_may_answer(true));
        assert!(!screen_may_answer(false));

        let first: IpAddr = "10.1.1.1".parse().unwrap();
        let id = create_request(first).unwrap();
        assert_eq!(create_request(first).unwrap(), id);
        assert_eq!(pending_for_screen().len(), 1);
        assert!(pending_for_screen()[0].message.contains("10.1.1.1"));

        decide(&id, false).unwrap();
        assert!(matches!(poll(&id), RequestView::Denied));
        assert!(decide(&id, true).is_err());
        assert!(pending_for_screen().is_empty());

        let second: IpAddr = "10.1.1.2".parse().unwrap();
        let id = create_request(second).unwrap();
        decide(&id, true).unwrap();
        let RequestView::Allowed(session) = poll(&id) else {
            panic!("allowed sign-in did not return a session");
        };
        assert!(credential_ok(&session, "not-the-token"));
        assert!(!credential_ok("short", "not-the-token"));
        assert!(credential_ok("not-the-token", "not-the-token"));

        {
            let mut open = gate();
            open.requests
                .iter_mut()
                .find(|request| request.id == id)
                .unwrap()
                .keep_until = Instant::now() - Duration::from_secs(1);
        }
        assert!(matches!(poll(&id), RequestView::Expired));
        assert!(credential_ok(&session, "not-the-token"));

        for host in ["10.2.0.1", "10.2.0.2", "10.2.0.3", "10.2.0.4"] {
            create_request(host.parse().unwrap()).unwrap();
        }
        assert!(create_request("10.2.0.5".parse().unwrap()).is_err());
    }
}
