//! Open an app like an OS opens a program: one press, and the app is there.
//!
//! Each app that works over plain http gets its own port on the front door,
//! on Keel's own addresses (the Keel screen, the house address, Away). That
//! address works on every device with no name and no router change. The
//! klaingar.com names stay for outside the house.
//!
//! When the app has an account that Keel made (Photos), Keel signs the
//! person in: the page asks Keel for a one-time ticket, the browser opens
//! `http://<keel>:<port>/keel-open/<ticket>`, the front door hands that one
//! path to Keel's local listener (127.0.0.1:9477), and Keel signs in to the
//! app on this computer and passes the app's own cookies back. Only someone
//! signed in to Keel gets a ticket. A ticket works once, for one app, for a
//! minute.
//!
//! Video and Passwords need a lock (camera, microphone, and the vault's
//! encryption only work on a locked page), so they keep their name.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::extract::Path as UrlPath;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// The front door's ports for apps. The firewall opens this range.
pub const FIRST_PORT: u16 = 9481;
pub const LAST_PORT: u16 = 9489;
const TICKET_MS: u64 = 60_000;

pub struct App {
    pub id: &'static str,
    pub title: &'static str,
    /// The front door port on Keel's addresses. 0: the app keeps its name.
    pub port: u16,
    /// Where the app listens on this computer.
    pub upstream: u16,
}

/// Ids are the same as the Apps page (`programs` in catalog.js).
pub const APPS: &[App] = &[
    App { id: "photos", title: "Photos", port: 9481, upstream: 2283 },
    App { id: "watch", title: "Watch", port: 9482, upstream: 8096 },
    App { id: "papers", title: "Papers", port: 9483, upstream: 1221 },
    App { id: "mail", title: "Mail", port: 9484, upstream: 3000 },
    App { id: "video", title: "Video", port: 0, upstream: 0 },
    App { id: "passwords", title: "Passwords", port: 0, upstream: 0 },
];

pub fn app(id: &str) -> Option<&'static App> {
    APPS.iter().find(|app| app.id == id)
}

#[derive(Serialize, Default)]
pub struct Launch {
    pub ok: bool,
    /// What the window opens: the sign-in hand-off, or the app itself.
    pub url: String,
    /// The app without a ticket, for "New tab".
    pub plain: String,
    /// The app only works on a locked page (camera, the vault).
    pub secure: bool,
    pub message: String,
    /// Keel has no sign-in for this app yet: the window asks once. The word
    /// for the account (Email or Name); empty when nothing is asked.
    pub ask: String,
    /// The app must start again first (Papers learns a new address only when
    /// it starts). The window closes it and opens it again.
    pub restart: bool,
}

/// How Keel gets the person into an app.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Entry {
    /// The app has no sign-in, or one Keel must not hold (the vault).
    Plain,
    /// Keel signs in with the app's account and hands its sign-in over.
    Account,
    /// The app asks the browser for a password (Mail's page). The front door
    /// lets through only someone Open sent, and gives the app the password.
    Gate,
}

fn entry(id: &str) -> Entry {
    match id {
        "photos" | "papers" | "watch" => Entry::Account,
        "mail" if !crate::versions::light("mail") => Entry::Gate,
        _ => Entry::Plain,
    }
}

/// The word the window uses for the account when it asks once.
fn account_word(id: &str) -> &'static str {
    match id {
        "papers" => "Email",
        _ => "Name",
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn tickets() -> &'static Mutex<HashMap<String, (&'static str, u64)>> {
    static TICKETS: OnceLock<Mutex<HashMap<String, (&'static str, u64)>>> = OnceLock::new();
    TICKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn new_ticket(id: &'static str) -> Option<String> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).ok()?;
    let ticket: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let now = now_ms();
    let mut map = tickets().lock().ok()?;
    map.retain(|_, (_, until)| *until > now);
    map.insert(ticket.clone(), (id, now + TICKET_MS));
    Some(ticket)
}

/// A ticket is used up the first time it is shown.
fn take_ticket(ticket: &str) -> Option<&'static str> {
    let mut map = tickets().lock().ok()?;
    let (id, until) = map.remove(ticket)?;
    (until > now_ms()).then_some(id)
}

/// The host the browser used to reach Keel, without the port, so the app
/// opens on the same address: 127.0.0.1 on the Keel screen, the house
/// address on a PC, the Away address away.
pub fn browser_host(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::HOST)?.to_str().ok()?.trim();
    let host = if let Some(rest) = raw.strip_prefix('[') {
        format!("[{}]", rest.split(']').next()?)
    } else {
        raw.rsplit_once(':').map(|(h, _)| h).unwrap_or(raw).to_string()
    };
    let plain = host.trim_matches(|c| c == '[' || c == ']');
    let ok = !plain.is_empty()
        && plain.len() <= 253
        && plain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':');
    ok.then(|| host.to_ascii_lowercase())
}

/// Builds the address the window opens. `ready_address` is the app's name
/// address (https://photos.<name>), used for the apps that need a lock.
pub fn launch(id: &str, host: &str, ready_address: &str) -> Launch {
    let Some(spec) = app(id) else {
        return Launch { message: "Keel does not know that app.".into(), ..Default::default() };
    };
    if spec.port == 0 {
        if ready_address.is_empty() {
            return Launch { message: format!("{} is not open yet.", spec.title), ..Default::default() };
        }
        return Launch {
            ok: true,
            url: ready_address.to_string(),
            plain: ready_address.to_string(),
            secure: true,
            ..Default::default()
        };
    }
    let how = entry(spec.id);
    if let Err(message) = door_port(spec, how == Entry::Gate) {
        return Launch { message, ..Default::default() };
    }
    let plain = format!("http://{host}:{}/", spec.port);
    // Only Keel's own numbered addresses go on Papers' list, so only those
    // are worth a restart; a name would never be on it.
    if spec.id == "papers" && host.parse::<std::net::Ipv4Addr>().is_ok() && !papers_trusts(host) {
        return Launch {
            plain,
            restart: true,
            message: "Papers is starting again so it opens here.".into(),
            ..Default::default()
        };
    }
    if how == Entry::Account && creds(spec.id).is_none() {
        return Launch {
            plain,
            message: format!("Sign in to {} once. Keel keeps the sign-in on this computer, so Open goes straight in next time.", spec.title),
            ask: account_word(spec.id).into(),
            ..Default::default()
        };
    }
    let url = match (how != Entry::Plain).then(|| new_ticket(spec.id)).flatten() {
        Some(ticket) => format!("http://{host}:{}/keel-open/{ticket}", spec.port),
        None => plain.clone(),
    };
    Launch { ok: true, url, plain, ..Default::default() }
}

/// Papers keeps a sign-in only on addresses it was started with
/// (TRUSTED_ORIGINS in papers-setup.sh). A Papers set up by an older Keel, or
/// before the house address changed, does not know this one yet.
fn papers_trusts(host: &str) -> bool {
    let want = format!("http://{host}:9483");
    let script = "podman inspect keel-papers --format '{{range .Config.Env}}{{println .}}{{end}}' 2>/dev/null | sed -n 's/^TRUSTED_ORIGINS=//p' | head -n 1 | sed 's/^/origins /'\n";
    match crate::storage::host_text("exec /bin/bash -s", &[], Some(script)) {
        Ok(line) => line.strip_prefix("origins ").unwrap_or("").split(',').any(|o| o.trim() == want),
        // When Keel cannot tell, it does not restart Papers on a guess.
        Err(_) => true,
    }
}

/// An app's sign-in, kept on this computer only (root, 0600).
#[derive(Serialize, serde::Deserialize, Clone)]
pub struct Cred {
    pub user: String,
    pub password: String,
}

const SIGNIN_DIR: &str = "/var/lib/keel/signin";

fn read_trim(path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The sign-in Keel holds for an app: one the person gave it, else the
/// account Keel made when it set the app up.
pub fn creds(id: &str) -> Option<Cred> {
    if let Ok(bytes) = std::fs::read(format!("{SIGNIN_DIR}/{id}.json")) {
        if let Ok(cred) = serde_json::from_slice::<Cred>(&bytes) {
            if !cred.user.is_empty() && !cred.password.is_empty() {
                return Some(cred);
            }
        }
    }
    match id {
        "photos" => Some(Cred {
            user: format!("photos@{}", crate::owned::load()?),
            password: read_trim("/var/lib/immich/account.pass")?,
        }),
        "mail" => Some(Cred { user: "mail".into(), password: read_trim("/var/lib/thunderbird/page.pass")? }),
        _ => None,
    }
}

fn save_creds(id: &str, cred: &Cred) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(SIGNIN_DIR).map_err(|_| "Keel could not keep the sign-in.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(SIGNIN_DIR, std::fs::Permissions::from_mode(0o700));
    }
    let body = serde_json::to_vec(cred).map_err(|_| "Keel could not keep the sign-in.".to_string())?;
    let path = format!("{SIGNIN_DIR}/{id}.json");
    let tmp = format!("{path}.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|_| "Keel could not keep the sign-in.".to_string())?;
    file.write_all(&body).map_err(|_| "Keel could not keep the sign-in.".to_string())?;
    std::fs::rename(&tmp, &path).map_err(|_| "Keel could not keep the sign-in.".to_string())
}

#[derive(Serialize)]
pub struct RememberedSignIn {
    pub id: &'static str,
    pub title: &'static str,
    pub user: String,
}

/// Only list user-supplied sign-ins. Keel-generated app credentials stay out
/// of this manager and are handled by the app's own card.
pub fn remembered_signins() -> Vec<RememberedSignIn> {
    ["photos", "papers", "watch"]
        .into_iter()
        .filter_map(|id| {
            let spec = app(id)?;
            let bytes = std::fs::read(format!("{SIGNIN_DIR}/{id}.json")).ok()?;
            let cred = serde_json::from_slice::<Cred>(&bytes).ok()?;
            (!cred.user.is_empty() && !cred.password.is_empty()).then_some(RememberedSignIn {
                id: spec.id,
                title: spec.title,
                user: cred.user,
            })
        })
        .collect()
}

/// Forget only the credential Keel saved; the app account and its data stay.
pub fn forget_signin(id: &str) -> Result<String, String> {
    let spec = app(id).ok_or("Keel does not know that app.")?;
    if entry(id) != Entry::Account {
        return Err(format!("Keel does not keep a sign-in for {}.", spec.title));
    }
    let path = std::path::Path::new(SIGNIN_DIR).join(format!("{id}.json"));
    if !path.exists() {
        return Err(format!("Keel has no saved sign-in for {}.", spec.title));
    }
    std::fs::remove_file(path).map_err(|_| format!("Keel could not forget the {} sign-in.", spec.title))?;
    Ok(format!("Keel forgot the {} sign-in. The app account and its data stay.", spec.title))
}

/// "Sign in once": Keel tries the name and password on the app itself, and
/// keeps them only when the app accepts them.
pub async fn remember(id: &str, user: &str, password: &str) -> Result<String, String> {
    let title = app(id).map(|a| a.title).ok_or("Keel does not know that app.")?;
    if entry(id) != Entry::Account {
        return Err(format!("Keel does not sign in to {title}."));
    }
    let cred = Cred { user: user.trim().to_string(), password: password.to_string() };
    if cred.user.is_empty() || cred.password.is_empty() {
        return Err(format!("Type the {} and the password.", account_word(id).to_lowercase()));
    }
    let works = match id {
        "photos" => photos_login(&cred).await.is_some(),
        "papers" => papers_login(&cred).await.is_some(),
        "watch" => watch_login(&cred).await.is_some(),
        _ => false,
    };
    if !works {
        return Err(format!("{title} did not accept that. Check the {} and password.", account_word(id).to_lowercase()));
    }
    let keep = cred.clone();
    let key = id.to_string();
    tokio::task::spawn_blocking(move || save_creds(&key, &keep))
        .await
        .map_err(|_| "Keel could not keep the sign-in.".to_string())??;
    Ok(format!("Keel keeps the {title} sign-in. Opening {title}."))
}

/// Makes sure the front door answers the app's port on Keel's addresses.
/// Cheap when it already does: the door is reloaded only when the port or the
/// Away address is missing from its file.
fn door_port(spec: &App, gate: bool) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err(format!("{} opens on the Keel computer.", spec.title));
    }
    let script = port_script(spec.port, spec.upstream, if gate { Some(spec.id) } else { None });
    let said = crate::storage::host_text("exec /bin/bash -s", &[], Some(&script));
    match said {
        Ok(line) if line == "ok" => Ok(()),
        Ok(line) => Err(format!("The front door did not open {}. {line}", spec.title)),
        Err(reason) => {
            eprintln!("keel: open {}: the front door said {reason}", spec.id);
            Err(reason)
        }
    }
}

/// `gate`: the app's id when the front door lets only Open's visitors in.
pub(crate) fn port_script(port: u16, upstream: u16, gate: Option<&str>) -> String {
    let extra = gate.map(|id| format!(" gate {id}")).unwrap_or_default();
    // The door's helper as this Keel writes it. The copy on the computer is
    // only rewritten when an app's name is sent, so a computer updated from an
    // older Keel still has a helper that does not know about ports.
    let helper = crate::names::DOOR_HELPER;
    format!(
        r#"set -uo pipefail
cfg=/var/lib/keel/caddy
[ -x /usr/local/bin/caddy ] && [ -d "$cfg" ] || {{ echo "The front door is not installed yet. Open Names on Apps first." >&2; exit 1; }}
install -d -m 0755 "$cfg/ports"
cat > /var/lib/keel/caddy-reload.sh.new <<'KEEL_DOOR_HELPER_END'
{helper}KEEL_DOOR_HELPER_END
chmod 755 /var/lib/keel/caddy-reload.sh.new
if ! cmp -s /var/lib/keel/caddy-reload.sh.new /var/lib/keel/caddy-reload.sh 2>/dev/null; then
  mv /var/lib/keel/caddy-reload.sh.new /var/lib/keel/caddy-reload.sh
  helper=new
else
  rm -f /var/lib/keel/caddy-reload.sh.new
  helper=same
fi
want='127.0.0.1:{upstream}{extra}'
have=$(cat "$cfg/ports/{port}" 2>/dev/null || true)
away=$(ip -4 addr show tailscale0 2>/dev/null | sed -n 's/.*inet \([0-9.]*\).*//p' | head -n 1)
reload=0
[ "$helper" = new ] && reload=1
[ "$have" = "$want" ] || {{ printf '%s
' "$want" > "$cfg/ports/{port}"; reload=1; }}
grep -q '^http://:{port} {{' "$cfg/Caddyfile" 2>/dev/null || reload=1
if [ -n "$away" ] && ! grep -q "bind .*$away" "$cfg/Caddyfile" 2>/dev/null; then reload=1; fi
if [ "$reload" = 1 ] && ! /var/lib/keel/caddy-reload.sh; then
  tail -n 3 /var/lib/keel/door.log 2>/dev/null >&2
  echo "The front door did not take the new port." >&2
  exit 1
fi
# The door answers on the port: /keel-open/ goes to Keel, whatever the app is doing.
i=0
door_up=0
while [ "$i" -lt 10 ]; do
  code=$(curl -s -o /dev/null --max-time 2 -w '%{{http_code}}' "http://127.0.0.1:{port}/keel-open/check" || true)
  case "$code" in ''|000) ;; *) door_up=1; break ;; esac
  i=$((i + 1))
  sleep 0.5
done
if [ "$door_up" != 1 ]; then
  journalctl -u keel-door -n 3 --no-pager -o cat 2>/dev/null >&2
  echo "The front door is not answering on port {port}." >&2
  exit 1
fi
# Check the app behind the door too. The saved setup status can outlive a
# stopped container; without this check the browser gets a blank 502 page.
i=0
code=000
while [ "$i" -lt 10 ]; do
  code=$(curl -s -o /dev/null --max-time 2 -w '%{{http_code}}' "http://127.0.0.1:{upstream}/" || true)
  case "$code" in 2??|3??|4??) echo ok; exit 0 ;; esac
  i=$((i + 1))
  sleep 0.5
done
if [ "$code" = 000 ]; then
  echo "The app page is not answering on this computer. Open tried to start it again; check Logs if it stays down." >&2
else
  echo "The app page returned HTTP $code. Check Logs to see why it is unavailable." >&2
fi
exit 1
"#
    )
}

/// What Close stops for each app: its systemd service (so it stays closed
/// after a restart, until Open) and its containers. Video has its own Close.
fn stop_parts(id: &str) -> Option<(&'static str, &'static [&'static str], &'static str)> {
    Some(match id {
        "photos" => ("keel-photos.service", &["keel-photos-server", "keel-photos-ml", "keel-photos-db", "keel-photos-redis"], "/run/keel/photos.json"),
        "watch" => ("", &["keel-jellyfin"], "/run/keel/jellyfin.json"),
        "papers" => ("keel-papers.service", &["keel-papers"], "/run/keel/papers.json"),
        "mail" => ("keel-mail.service", &["keel-mail"], "/run/keel/mail.json"),
        "passwords" => ("keel-passwords.service", &["keel-passwords"], "/run/keel/passwords.json"),
        _ => return None,
    })
}

pub(crate) fn stop_script(service: &str, containers: &[&str]) -> String {
    let mut script = String::from("set -uo pipefail\n");
    if !service.is_empty() {
        script.push_str(&format!("systemctl disable --now {service} >/dev/null 2>&1 || true\n"));
    }
    script.push_str(&format!("podman stop -t 20 {} >/dev/null 2>&1 || true\n", containers.join(" ")));
    script.push_str("echo ok\n");
    script
}

/// Close: the app stops and stays stopped until Open. Its program, settings
/// and data stay on this computer.
pub fn stop(id: &str) -> Result<String, String> {
    let title = app(id).map(|a| a.title).ok_or("Keel does not know that app.")?;
    if !cfg!(target_os = "linux") {
        return Err(format!("{title} runs on the Keel computer."));
    }
    if id == "video" {
        let closed = if crate::versions::light("video") { crate::galene::close() } else { crate::jitsi::close() };
        return closed.map(|_| "Video is closed. Open starts it again.".to_string());
    }
    let (service, containers, progress) = stop_parts(id).ok_or("Keel does not know that app.")?;
    let opening = std::fs::read(progress)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .map(|saved| saved.get("state").and_then(|v| v.as_str()) == Some("running"))
        .unwrap_or(false);
    if opening {
        return Err(format!("{title} is still opening. Try Close again in a moment."));
    }
    crate::storage::host_text("exec /bin/bash -s", &[], Some(&stop_script(service, containers)))
        .map_err(|_| format!("Keel could not close {title}. Try Close again."))?;
    let _ = std::fs::remove_file(progress);
    Ok(format!("{title} is closed. Open starts it again."))
}

/// Which apps are running now, asked of the computer itself. The card's own
/// note lives in /run and is gone after a restart, while an app may have
/// started again with the computer.
pub fn running() -> Vec<&'static str> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    let script = "names=$(podman ps --format '{{.Names}}' 2>/dev/null | tr '\\n' ' ')\n\
        systemctl is-active --quiet keel-galene.service && names=\"$names keel-galene\"\n\
        echo \"up $names\"\n";
    let Ok(line) = crate::storage::host_text("exec /bin/bash -s", &[], Some(script)) else {
        return Vec::new();
    };
    let names: Vec<&str> = line.strip_prefix("up").unwrap_or("").split_whitespace().collect();
    let mut up = Vec::new();
    for (id, main) in [
        ("photos", "keel-photos-server"),
        ("watch", "keel-jellyfin"),
        ("papers", "keel-papers"),
        ("mail", "keel-mail"),
        ("passwords", "keel-passwords"),
    ] {
        if names.contains(&main) {
            up.push(id);
        }
    }
    if names.contains(&"keel-galene") || names.contains(&"keel-jitsi-web") {
        up.push("video");
    }
    up
}

/// `GET /keel-open/{ticket}` on the local listener, reached only through the
/// front door on an app's port. Signs in and sends the browser to the app.
pub async fn hand_off(UrlPath(ticket): UrlPath<String>) -> Response {
    let to_app = |cookies: Vec<HeaderValue>| {
        let mut response = (StatusCode::FOUND, [(header::LOCATION, "/")]).into_response();
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        for cookie in cookies {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
        response
    };
    let Some(id) = take_ticket(&ticket) else {
        return to_app(Vec::new());
    };
    let cred = tokio::task::spawn_blocking(move || creds(id)).await.ok().flatten();
    match (id, cred) {
        ("photos", Some(cred)) => to_app(photos_login(&cred).await.unwrap_or_default()),
        ("papers", Some(cred)) => to_app(papers_login(&cred).await.unwrap_or_default()),
        ("watch", Some(cred)) => match watch_login(&cred).await {
            Some(signed) => watch_page(&signed),
            None => to_app(Vec::new()),
        },
        ("mail", _) => to_app(new_gate(id).into_iter().collect()),
        _ => to_app(Vec::new()),
    }
}

fn http() -> Option<reqwest::Client> {
    reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().ok()
}

fn app_cookies(answer: &reqwest::Response) -> Vec<HeaderValue> {
    answer
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| plain_cookie(value.to_str().ok()?))
        .collect()
}

/// Signs in to Photos (Immich) on this computer and returns its own cookies.
/// The password never reaches the browser.
async fn photos_login(cred: &Cred) -> Option<Vec<HeaderValue>> {
    let answer = http()?
        .post("http://127.0.0.1:2283/api/auth/login")
        .json(&serde_json::json!({ "email": cred.user, "password": cred.password }))
        .send()
        .await
        .ok()?;
    if !answer.status().is_success() {
        return None;
    }
    let mut cookies = app_cookies(&answer);
    if cookies.is_empty() {
        // An older Photos answers with the token only; write its cookies.
        let body: serde_json::Value = answer.json().await.ok()?;
        let token = body.get("accessToken")?.as_str()?;
        if !safe_token(token) {
            return None;
        }
        for line in [
            format!("immich_access_token={token}; Path=/; HttpOnly; SameSite=Lax"),
            "immich_auth_type=password; Path=/; HttpOnly; SameSite=Lax".to_string(),
            "immich_is_authenticated=true; Path=/; SameSite=Lax".to_string(),
        ] {
            cookies.push(HeaderValue::from_str(&line).ok()?);
        }
    }
    Some(cookies)
}

/// Signs in to Papers (Papra, Better Auth) and returns its session cookie.
/// The Keel screen's address is always on Papers' trusted list
/// (papers-setup.sh), so Keel asks from there.
async fn papers_login(cred: &Cred) -> Option<Vec<HeaderValue>> {
    let answer = http()?
        .post("http://127.0.0.1:1221/api/auth/sign-in/email")
        .header(header::ORIGIN, "http://127.0.0.1:9483")
        .json(&serde_json::json!({ "email": cred.user, "password": cred.password }))
        .send()
        .await
        .ok()?;
    if !answer.status().is_success() {
        return None;
    }
    let cookies = app_cookies(&answer);
    (!cookies.is_empty()).then_some(cookies)
}

pub(crate) struct WatchSignIn {
    token: String,
    user: String,
    server: String,
}

fn safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Signs in to Watch (Jellyfin) as the house account.
async fn watch_login(cred: &Cred) -> Option<WatchSignIn> {
    let answer = http()?
        .post("http://127.0.0.1:8096/Users/AuthenticateByName")
        .header(
            header::AUTHORIZATION,
            r#"MediaBrowser Client="Keel", Device="Keel window", DeviceId="keel-open", Version="1""#,
        )
        .json(&serde_json::json!({ "Username": cred.user, "Pw": cred.password }))
        .send()
        .await
        .ok()?;
    if !answer.status().is_success() {
        return None;
    }
    let body: serde_json::Value = answer.json().await.ok()?;
    let signed = WatchSignIn {
        token: body.get("AccessToken")?.as_str()?.to_string(),
        user: body.get("User")?.get("Id")?.as_str()?.to_string(),
        server: body.get("ServerId")?.as_str()?.to_string(),
    };
    (safe_token(&signed.token) && safe_token(&signed.user) && safe_token(&signed.server)).then_some(signed)
}

/// Watch keeps its sign-in in the browser, not in a cookie, so Keel answers
/// with a tiny page on Watch's own address that writes it there and then
/// opens Watch.
pub(crate) fn watch_page(signed: &WatchSignIn) -> Response {
    let page = format!(
        r#"<!doctype html><meta charset="utf-8"><title>Watch</title><script>
(function () {{
  var server = {{ ManualAddress: location.origin, manualAddressOnly: true, Id: "{server}", Name: "Watch",
    UserId: "{user}", AccessToken: "{token}", DateLastAccessed: Date.now(), LastConnectionMode: 2 }};
  try {{ localStorage.setItem("jellyfin_credentials", JSON.stringify({{ Servers: [server] }})); }} catch (e) {{}}
  location.replace("/web/#/home.html");
}})();
</script>"#,
        server = signed.server,
        user = signed.user,
        token = signed.token,
    );
    let mut response = (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], page).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

// The Mail gate: Open gives the browser a gate cookie for Mail's port; the
// front door asks `/keel-gate/mail` before every request there, and Keel
// answers with Mail's own password for the app, never for the browser.
const GATE_MS: u64 = 12 * 60 * 60 * 1000;

fn gates() -> &'static Mutex<HashMap<String, (&'static str, u64)>> {
    static GATES: OnceLock<Mutex<HashMap<String, (&'static str, u64)>>> = OnceLock::new();
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn new_gate(id: &'static str) -> Option<HeaderValue> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).ok()?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let now = now_ms();
    let mut map = gates().lock().ok()?;
    map.retain(|_, (_, until)| *until > now);
    map.insert(token.clone(), (id, now + GATE_MS));
    HeaderValue::from_str(&format!("keel_gate_{id}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=43200")).ok()
}

fn gate_open(id: &str, cookie_header: &str) -> bool {
    let name = format!("keel_gate_{id}=");
    let now = now_ms();
    let Ok(map) = gates().lock() else { return false };
    cookie_header
        .split(';')
        .filter_map(|part| part.trim().strip_prefix(name.as_str()))
        .any(|token| map.get(token).is_some_and(|(gate, until)| *gate == id && *until > now))
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `GET /keel-gate/{id}`, asked by the front door (forward_auth).
pub async fn gate(UrlPath(id): UrlPath<String>, headers: HeaderMap) -> Response {
    let cookies = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let title = app(&id).map(|a| a.title).unwrap_or("This app");
    if entry(&id) == Entry::Gate && gate_open(&id, cookies) {
        let cred = {
            let key = id.clone();
            tokio::task::spawn_blocking(move || creds(&key)).await.ok().flatten()
        };
        if let Some(cred) = cred {
            let basic = format!("Basic {}", base64(format!("{}:{}", cred.user, cred.password).as_bytes()));
            if let Ok(value) = HeaderValue::from_str(&basic) {
                let mut response = StatusCode::OK.into_response();
                response.headers_mut().insert(header::AUTHORIZATION, value);
                return response;
            }
        }
    }
    let page = format!(
        r#"<!doctype html><meta charset="utf-8"><title>{title}</title><body style="font:16px system-ui;padding:40px;background:#101112;color:#f1eee8">Open {title} from Keel's Apps page.</body>"#
    );
    (StatusCode::UNAUTHORIZED, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], page).into_response()
}

/// The app's cookie, made to work on Keel's own address: no Domain (the
/// browser keeps it for the address it is on) and not Secure (the port is
/// plain http on the house network).
fn plain_cookie(line: &str) -> Option<HeaderValue> {
    let mut parts = line.split(';').map(str::trim).filter(|part| !part.is_empty());
    let first = parts.next()?;
    if !first.contains('=') {
        return None;
    }
    let mut kept = vec![first.to_string()];
    for part in parts {
        let key = part.split('=').next().unwrap_or("").trim().to_ascii_lowercase();
        if key == "domain" || key == "secure" {
            continue;
        }
        if key == "samesite" && part.to_ascii_lowercase().ends_with("none") {
            kept.push("SameSite=Lax".into());
            continue;
        }
        kept.push(part.to_string());
    }
    HeaderValue::from_str(&kept.join("; ")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_are_in_the_range_the_firewall_opens() {
        for app in APPS.iter().filter(|a| a.port != 0) {
            assert!((FIRST_PORT..=LAST_PORT).contains(&app.port), "{}", app.id);
            assert_ne!(app.port, 9478);
        }
        let mut ports: Vec<u16> = APPS.iter().map(|a| a.port).filter(|p| *p != 0).collect();
        ports.sort();
        ports.dedup();
        assert_eq!(ports.len(), APPS.iter().filter(|a| a.port != 0).count());
    }

    #[test]
    fn a_ticket_works_once() {
        let ticket = new_ticket("photos").unwrap();
        assert_eq!(ticket.len(), 48);
        assert_eq!(take_ticket(&ticket), Some("photos"));
        assert_eq!(take_ticket(&ticket), None);
        assert_eq!(take_ticket("nope"), None);
    }

    #[test]
    fn the_app_opens_on_the_address_the_browser_used() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("192.168.0.191:9478"));
        assert_eq!(browser_host(&headers).as_deref(), Some("192.168.0.191"));
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:9478"));
        assert_eq!(browser_host(&headers).as_deref(), Some("127.0.0.1"));
        headers.insert(header::HOST, HeaderValue::from_static("keel.tail1234.ts.net"));
        assert_eq!(browser_host(&headers).as_deref(), Some("keel.tail1234.ts.net"));
        headers.insert(header::HOST, HeaderValue::from_static("[fd7a::1]:9478"));
        assert_eq!(browser_host(&headers).as_deref(), Some("[fd7a::1]"));
        headers.insert(header::HOST, HeaderValue::from_static("evil\"/x"));
        assert_eq!(browser_host(&headers), None);
    }

    #[test]
    fn video_and_passwords_keep_their_locked_name() {
        let open = launch("video", "192.168.0.191", "https://video.klaingar.com");
        assert!(open.ok && open.secure);
        assert_eq!(open.url, "https://video.klaingar.com");
        assert!(!launch("passwords", "192.168.0.191", "").ok);
    }

    #[test]
    fn the_app_cookie_stays_on_keel_s_address() {
        let cookie = plain_cookie("immich_access_token=abc; Path=/; Domain=klaingar.com; Secure; HttpOnly; SameSite=None").unwrap();
        assert_eq!(cookie.to_str().unwrap(), "immich_access_token=abc; Path=/; HttpOnly; SameSite=Lax");
        assert!(plain_cookie("junk").is_none());
    }

    /// Needs a pretend Photos on 127.0.0.1:2283, /var/lib/immich/account.pass,
    /// and KEEL_DATA with owned-name.json. Run with --ignored.
    /// Needs pretend Papers on 1221 and Watch on 8096 (see the release notes
    /// in activeContext.md). Run with --ignored, as root.
    #[tokio::test]
    #[ignore]
    async fn papers_watch_and_mail_hand_offs() {
        // Sign in once: a wrong password is not kept, a right one is.
        assert!(remember("papers", "me@x.com", "wrong").await.is_err());
        assert!(remember("papers", "me@x.com", "pp").await.is_ok());
        assert!(remember("watch", "house", "jj").await.is_ok());
        let papers = hand_off(UrlPath(new_ticket("papers").unwrap())).await;
        let cookies: Vec<_> = papers.headers().get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_string()).collect();
        assert_eq!(cookies, vec!["better-auth.session_token=sess9; Path=/; HttpOnly; SameSite=Lax".to_string()]);
        let watch = hand_off(UrlPath(new_ticket("watch").unwrap())).await;
        assert_eq!(watch.status(), StatusCode::OK);
        let body = axum::body::to_bytes(watch.into_body(), 10_000).await.unwrap();
        let page = String::from_utf8(body.to_vec()).unwrap();
        assert!(page.contains(r#"AccessToken: "tokJ""#) && page.contains(r#"UserId: "uid7""#) && page.contains(r#"Id: "srv3""#));
        assert!(page.contains("jellyfin_credentials"));
        // Mail: the gate cookie opens the gate, which hands Mail its password.
        std::fs::create_dir_all("/var/lib/thunderbird").unwrap();
        std::fs::write("/var/lib/thunderbird/page.pass", "mpw\n").unwrap();
        let mail = hand_off(UrlPath(new_ticket("mail").unwrap())).await;
        let gate_cookie = mail.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().split(';').next().unwrap().to_string();
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_str(&gate_cookie).unwrap());
        let open = gate(UrlPath("mail".into()), headers).await;
        assert_eq!(open.status(), StatusCode::OK);
        assert_eq!(open.headers().get(header::AUTHORIZATION).unwrap(), &format!("Basic {}", base64(b"mail:mpw")));
        let shut = gate(UrlPath("mail".into()), HeaderMap::new()).await;
        assert_eq!(shut.status(), StatusCode::UNAUTHORIZED);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata("/var/lib/keel/signin/papers.json").unwrap();
            assert_eq!(mode.permissions().mode() & 0o777, 0o600);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn the_hand_off_passes_photos_cookies_and_goes_to_the_app() {
        let ticket = new_ticket("photos").unwrap();
        let response = hand_off(UrlPath(ticket.clone())).await;
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/");
        let cookies: Vec<_> = response.headers().get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_string()).collect();
        println!("{cookies:?}");
        assert!(cookies.iter().any(|c| c.starts_with("immich_access_token=")));
        assert!(cookies.iter().all(|c| !c.contains("Secure") && !c.contains("Domain")));
        let again = hand_off(UrlPath(ticket)).await;
        assert!(again.headers().get(header::SET_COOKIE).is_none());
    }

    #[test]
    fn close_stops_the_service_and_its_containers() {
        let (service, containers, progress) = stop_parts("photos").unwrap();
        let script = stop_script(service, containers);
        assert!(script.contains("systemctl disable --now keel-photos.service"));
        assert!(script.contains("podman stop -t 20 keel-photos-server keel-photos-ml keel-photos-db keel-photos-redis"));
        assert_eq!(progress, "/run/keel/photos.json");
        assert!(!script.contains("podman rm"));
        let watch = stop_script("", stop_parts("watch").unwrap().1);
        assert!(!watch.contains("systemctl"));
        for id in ["photos", "watch", "papers", "mail", "passwords"] {
            assert!(stop_parts(id).is_some(), "{id}");
        }
    }

    #[test]
    fn mail_s_gate_answers_only_for_open_s_cookie() {
        let cookie = new_gate("mail").unwrap();
        let token = cookie.to_str().unwrap().split(';').next().unwrap().strip_prefix("keel_gate_mail=").unwrap().to_string();
        assert!(gate_open("mail", &format!("other=1; keel_gate_mail={token}")));
        assert!(!gate_open("mail", "keel_gate_mail=nope"));
        assert!(!gate_open("photos", &format!("keel_gate_photos={token}")));
        assert!(cookie.to_str().unwrap().contains("HttpOnly"));
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b"mail:abc"), "bWFpbDphYmM=");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
    }

    #[test]
    fn watch_s_page_writes_its_sign_in_and_opens_watch() {
        let response = watch_page(&WatchSignIn { token: "tok1".into(), user: "u1".into(), server: "s1".into() });
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(header::CACHE_CONTROL).unwrap(), "no-store");
        assert!(!safe_token("a\"</script>"));
    }

    #[test]
    fn the_port_script_reloads_only_when_needed() {
        let script = port_script(9481, 2283, None);
        assert!(port_script(9484, 3000, Some("mail")).contains("want='127.0.0.1:3000 gate mail'"));
        assert!(script.contains("want='127.0.0.1:2283'"));
        assert!(script.contains("\"$cfg/ports/9481\""));
        assert!(script.contains("grep -q '^http://:9481 {'"));
        assert!(script.contains("cat > /var/lib/keel/caddy-reload.sh.new <<'KEEL_DOOR_HELPER_END'"));
        assert!(script.contains("http://127.0.0.1:9481/keel-open/check"));
        assert!(script.contains(crate::names::DOOR_HELPER.lines().next().unwrap()));
        assert!(!script.contains("9478"));
        assert!(!script.contains("0.0.0.0"));
    }
}
