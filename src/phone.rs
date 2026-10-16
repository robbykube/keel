//! A phone message when something is red.
//!
//! The phone uses the free ntfy app and subscribes to this computer over Away.
//! The words stay here. Yellow, a short bar, and an unknown disk do not send.

use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tokio::sync::{broadcast, Notify};

const TOPIC: &str = "keel";
const PORT: u16 = 9479;
const FLAG: &str = "phone.on";

static GO: Notify = Notify::const_new();
static NEXT: AtomicU64 = AtomicU64::new(1);
static TOLD: Mutex<Vec<String>> = Mutex::new(Vec::new());
static RECENT: Mutex<VecDeque<Stored>> = Mutex::new(VecDeque::new());
static BUS: OnceLock<broadcast::Sender<String>> = OnceLock::new();

struct Stored {
    id: String,
    json: String,
}

#[derive(Clone)]
pub struct Note {
    pub id: String,
    pub text: String,
}

pub fn note(title: &str, why: &str, evidence: &str) -> Note {
    let id = format!("{title}|{evidence}");
    let text = if evidence.is_empty() {
        format!("{title}\n{why}")
    } else {
        format!("{title}\n{why}\n{evidence}")
    };
    Note { id, text }
}

/// A message only for a red item the phone has not already heard.
pub fn take_new(reds: &[Note]) -> Option<String> {
    let mut told = TOLD.lock().unwrap_or_else(|poison| poison.into_inner());
    let fresh: Vec<&Note> = reds.iter().filter(|note| !told.iter().any(|id| id == &note.id)).collect();
    *told = reds.iter().map(|note| note.id.clone()).collect();
    if fresh.is_empty() {
        return None;
    }
    Some(
        fresh
            .iter()
            .map(|note| note.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

pub fn publish(text: &str) {
    let id = NEXT.fetch_add(1, Ordering::Relaxed).to_string();
    let json = serde_json::json!({
        "id": id,
        "time": unix_secs(),
        "event": "message",
        "topic": TOPIC,
        "message": text,
    })
    .to_string();
    {
        let mut recent = RECENT.lock().unwrap_or_else(|poison| poison.into_inner());
        recent.push_back(Stored {
            id: id.clone(),
            json: json.clone(),
        });
        while recent.len() > 20 {
            recent.pop_front();
        }
    }
    let _ = bus().send(json);
}

pub fn kick() {
    GO.notify_one();
}

pub fn wanted() -> bool {
    flag_path().is_file()
}

pub fn view() -> serde_json::Value {
    match away_ip() {
        Some(ip) if wanted() => serde_json::json!({
            "on": true,
            "server": format!("http://{ip}:{PORT}"),
            "topic": TOPIC,
        }),
        _ => serde_json::json!({ "on": false, "server": "", "topic": TOPIC }),
    }
}

pub fn enable() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("This is on the Keel computer.".into());
    }
    let ip = away_ip().ok_or_else(|| "Connect Away first. The phone reaches this computer that way.".to_string())?;
    remember()?;
    Ok(format!(
        "In the free ntfy app, set the server to http://{ip}:{PORT} and subscribe to {TOPIC}. The phone must be on Away. On an iPhone, open the app to see it. An Android phone can show it while Away is connected."
    ))
}

pub async fn serve() {
    loop {
        if wanted() {
            if let Some(ip) = away_ip() {
                if let Err(error) = listen(ip).await {
                    eprintln!("keel: {error}");
                }
            }
        }
        tokio::select! {
            _ = GO.notified() => {}
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
        }
    }
}

pub fn tailscale_ip(ip: &str) -> bool {
    let Ok(addr) = ip.parse::<Ipv4Addr>() else {
        return false;
    };
    let [a, b, _, _] = addr.octets();
    a == 100 && (64..128).contains(&b)
}

fn away_ip() -> Option<String> {
    let status = crate::overlay::status();
    status.overlay_ip.filter(|ip| tailscale_ip(ip))
}

fn flag_path() -> std::path::PathBuf {
    let root = std::env::var("KEEL_DATA").unwrap_or_else(|_| {
        if cfg!(target_os = "linux") {
            "/var/lib/keel".into()
        } else {
            "data".into()
        }
    });
    std::path::PathBuf::from(root).join(FLAG)
}

fn remember() -> Result<(), String> {
    let path = flag_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "Keel could not remember the phone.".to_string())?;
    }
    std::fs::write(&path, b"on\n").map_err(|_| "Keel could not remember the phone.".to_string())
}

fn bus() -> &'static broadcast::Sender<String> {
    BUS.get_or_init(|| broadcast::channel(32).0)
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn frame(event: &str) -> String {
    serde_json::json!({
        "id": NEXT.fetch_add(1, Ordering::Relaxed).to_string(),
        "time": unix_secs(),
        "event": event,
        "topic": TOPIC,
    })
    .to_string()
}

async fn listen(ip: String) -> Result<(), String> {
    let addr: SocketAddr = SocketAddr::new(
        ip.parse::<IpAddr>().map_err(|_| "The phone address was not kept.".to_string())?,
        PORT,
    );
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|_| format!("The phone could not listen on {ip}."))?;
    let app = Router::new()
        .route("/{topic}/json", get(json_poll))
        .route("/{topic}/ws", get(socket));
    axum::serve(listener, app).await.map_err(|_| "The phone listener stopped.".to_string())
}

#[derive(Deserialize, Default)]
struct Since {
    #[serde(default)]
    since: String,
}

async fn json_poll(Path(topic): Path<String>, Query(since): Query<Since>) -> impl IntoResponse {
    if topic != TOPIC {
        return (axum::http::StatusCode::NOT_FOUND, "").into_response();
    }
    let body = recent_after(&since.since).join("\n");
    let body = if body.is_empty() { body } else { format!("{body}\n") };
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/x-ndjson",
        )],
        body,
    )
        .into_response()
}

async fn socket(Path(topic): Path<String>, ws: WebSocketUpgrade) -> impl IntoResponse {
    if topic != TOPIC {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }
    ws.on_upgrade(feed).into_response()
}

async fn feed(mut socket: WebSocket) {
    let _ = socket.send(Message::Text(frame("open").into())).await;
    for line in recent_after("all") {
        if socket.send(Message::Text(line.into())).await.is_err() {
            return;
        }
    }
    let mut rx = bus().subscribe();
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(20));
    tick.tick().await;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if socket.send(Message::Text(frame("keepalive").into())).await.is_err() {
                    return;
                }
            }
            incoming = rx.recv() => {
                let Ok(line) = incoming else { continue; };
                if socket.send(Message::Text(line.into())).await.is_err() {
                    return;
                }
            }
        }
    }
}

fn recent_after(since: &str) -> Vec<String> {
    let recent = RECENT.lock().unwrap_or_else(|poison| poison.into_inner());
    if since.is_empty() || since == "all" {
        return recent.iter().map(|item| item.json.clone()).collect();
    }
    let mut skip = true;
    let mut out = Vec::new();
    for item in recent.iter() {
        if skip {
            if item.id == since {
                skip = false;
            }
            continue;
        }
        out.push(item.json.clone());
    }
    if skip {
        recent.iter().map(|item| item.json.clone()).collect()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_away_address_is_accepted() {
        assert!(tailscale_ip("100.113.90.33"));
        assert!(!tailscale_ip("192.168.0.191"));
        assert!(!tailscale_ip("127.0.0.1"));
        assert!(!tailscale_ip("keel.tailnet.ts.net"));
    }

    #[test]
    fn a_red_item_is_sent_once_until_it_changes() {
        let first = note("A disk is failing", "Copy the files off it, then replace that disk.", "WD (sdb)");
        let again = first.clone();
        let missing = note(
            "A Keel disk is not connected",
            "Check the power and cable.",
            "Files",
        );
        assert!(fresh(&[first.clone()]).unwrap().contains("A disk is failing"));
        assert!(fresh(&[again]).is_none());
        let both = fresh(&[
            note("A disk is failing", "Copy the files off it, then replace that disk.", "WD (sdb)"),
            missing,
        ])
        .unwrap();
        assert!(both.contains("not connected"));
        assert!(!both.contains("A disk is failing"));
        assert!(fresh(&[]).is_none());
    }

    fn fresh(reds: &[Note]) -> Option<String> {
        take_new(reds)
    }
}
