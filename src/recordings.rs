//! Call recordings. In a Video call, each person's browser records its own
//! camera and microphone at full quality and sends it here in pieces, through
//! the front door (`video.<name>/keel-recording/…` to 127.0.0.1:9477). A slow
//! connection only makes the pieces arrive later; they are kept on the
//! browser's side until Keel has them. Keel keeps one file per person in
//! Media/Recordings/<call>/, and when the call's recordings are finished it
//! makes Everyone.mp4 with everyone side by side, each at the moment they
//! started.
//!
//! This listener takes recordings only while Video is open. It also answers
//! Open's two paths (launch.rs): `/keel-open/<ticket>`, the one-time sign-in
//! hand-off, and `/keel-gate/<app>`, the front door's question before Mail.
//! No page and no settings.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::body::Bytes;
use axum::extract::Path as UrlPath;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

/// Only the front door talks to this port.
pub const PORT: u16 = 9477;
const MAX_PIECE: usize = 16 * 1024 * 1024;
const MAX_PERSON: u64 = 8 * 1024 * 1024 * 1024;
/// Recording stops before the files disk is this full.
const KEEP_FREE: u64 = 2 * 1024 * 1024 * 1024;
/// A recording that sent nothing for this long is finished as it is.
const QUIET_MS: u64 = 30 * 60 * 1000;
/// A new recording joins a call folder from the same room this recent.
const SAME_CALL_MS: u64 = 15 * 60 * 1000;
/// Everyone's video is made once the call's recordings have been finished
/// and quiet this long.
const SETTLE_MS: u64 = 2 * 60 * 1000;
static COMBINING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const MAX_OPEN: usize = 16;
const PARTS: &str = ".parts";
const CALL_FILE: &str = ".call.json";
pub const COMBINED: &str = "Everyone.mp4";

fn recordings_dir() -> PathBuf {
    if cfg!(target_os = "linux") && std::env::var_os("KEEL_RECORDINGS").is_none() {
        PathBuf::from("/srv/keel/files/Media/Recordings")
    } else {
        std::env::var_os("KEEL_RECORDINGS").map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into()))
                .join("mock-files/Media/Recordings")
        })
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// One call: a folder, the room it came from, when it started.
#[derive(Serialize, Deserialize, Clone, Default)]
struct Call {
    room: String,
    started_ms: u64,
    last_ms: u64,
    /// "", "making", "done", or what went wrong.
    #[serde(default)]
    combined: String,
    /// How many people's files the last combined video held.
    #[serde(default)]
    people: usize,
}

/// One person's recording in a call.
#[derive(Serialize, Deserialize, Clone)]
struct Take {
    token: String,
    call: String,
    person: String,
    /// "webm" or "mp4", from what the browser records.
    kind: String,
    /// When it started, after the call started.
    offset_ms: u64,
    last_ms: u64,
    bytes: u64,
    done: bool,
}

#[derive(Default)]
struct Book {
    loaded: bool,
    takes: BTreeMap<String, Take>,
}

fn book() -> std::sync::MutexGuard<'static, Book> {
    static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
    let mut book = BOOK.get_or_init(|| Mutex::new(Book::default())).lock().unwrap_or_else(|e| e.into_inner());
    if !book.loaded {
        book.loaded = true;
        book.takes = load_takes(&recordings_dir());
    }
    book
}

/// Takes still being sent, read back after a restart.
fn load_takes(root: &Path) -> BTreeMap<String, Take> {
    let mut takes = BTreeMap::new();
    let Ok(calls) = fs::read_dir(root) else {
        return takes;
    };
    for call in calls.flatten() {
        let Ok(parts) = fs::read_dir(call.path().join(PARTS)) else {
            continue;
        };
        for part in parts.flatten() {
            let id = part.file_name().to_string_lossy().to_string();
            if let Some(take) = fs::read(part.path().join("take.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Take>(&bytes).ok())
            {
                takes.insert(id, take);
            }
        }
    }
    takes
}

pub fn router() -> Router {
    Router::new()
        .route("/keel-recording/recorder.js", get(recorder_js))
        .route("/keel-recording/room/{room}", get(room_view))
        .route("/keel-recording/room/{room}/start", post(room_start))
        .route("/keel-recording/room/{room}/stop", post(room_stop))
        .route("/keel-recording/start", post(start))
        .route("/keel-recording/{id}", get(have))
        .route("/keel-recording/{id}/{seq}", put(piece))
        .route("/keel-recording/{id}/done", post(done))
        // Open's sign-in hand-off, on an app's port (see launch.rs).
        .route("/keel-open/{ticket}", get(crate::launch::hand_off))
        .route("/keel-gate/{id}", get(crate::launch::gate))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_PIECE + 1024))
}

pub async fn serve() {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], PORT));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            let _ = axum::serve(listener, router()).await;
        }
        Err(error) => eprintln!("keel: recordings could not listen on {addr}: {error}"),
    }
}

type Answer = (StatusCode, Json<serde_json::Value>);

/// The script the meeting page loads (custom-config.js in jitsi-setup.sh).
async fn recorder_js() -> ([(axum::http::HeaderName, &'static str); 2], &'static str) {
    (
        [
            (axum::http::header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
        ],
        include_str!("../web/recorder.js"),
    )
}

/// Whether each room is being recorded, kept for everyone in the call: any
/// one of them presses Record, and every browser in the room starts.
fn rooms() -> std::sync::MutexGuard<'static, BTreeMap<String, u64>> {
    static ROOMS: OnceLock<Mutex<BTreeMap<String, u64>>> = OnceLock::new();
    ROOMS.get_or_init(|| Mutex::new(BTreeMap::new())).lock().unwrap_or_else(|e| e.into_inner())
}

fn video_open() -> bool {
    crate::jitsi::view().ready || crate::galene::view().ready || std::env::var_os("KEEL_RECORDINGS").is_some()
}

async fn room_view(UrlPath(room): UrlPath<String>) -> Answer {
    let room = clean_name(&room, "Call");
    let since = if video_open() { rooms().get(&room).copied() } else { None };
    (StatusCode::OK, Json(serde_json::json!({ "recording": since.is_some(), "since": since.unwrap_or(0) })))
}

async fn room_start(UrlPath(room): UrlPath<String>) -> Answer {
    if !video_open() {
        return no(StatusCode::CONFLICT, "Video is closed, so Keel is not taking recordings.");
    }
    let room = clean_name(&room, "Call");
    let mut rooms = rooms();
    if rooms.len() >= MAX_OPEN && !rooms.contains_key(&room) {
        return no(StatusCode::TOO_MANY_REQUESTS, "Too many calls are being recorded at once.");
    }
    rooms.entry(room).or_insert_with(now_ms);
    (StatusCode::OK, Json(serde_json::json!({ "ok": true, "recording": true })))
}

async fn room_stop(UrlPath(room): UrlPath<String>) -> Answer {
    rooms().remove(&clean_name(&room, "Call"));
    (StatusCode::OK, Json(serde_json::json!({ "ok": true, "recording": false })))
}

fn no(status: StatusCode, message: &str) -> Answer {
    (status, Json(serde_json::json!({ "ok": false, "message": message })))
}

#[derive(Deserialize)]
struct StartAsk {
    room: String,
    name: String,
    #[serde(default)]
    kind: String,
}

async fn start(Json(ask): Json<StartAsk>) -> Answer {
    if !video_open() {
        return no(StatusCode::CONFLICT, "Video is closed, so Keel is not taking recordings.");
    }
    match tokio::task::spawn_blocking(move || begin(&recordings_dir(), &ask.room, &ask.name, &ask.kind, now_ms())).await {
        Ok(Ok((id, take))) => (StatusCode::OK, Json(serde_json::json!({ "ok": true, "id": id, "token": take.token, "folder": take.call }))),
        Ok(Err(message)) => no(StatusCode::BAD_REQUEST, &message),
        Err(_) => no(StatusCode::INTERNAL_SERVER_ERROR, "Keel could not start the recording."),
    }
}

/// Starts one person's recording: finds or makes the call's folder, picks a
/// file name nobody in that call has, and keeps the take on disk.
fn begin(root: &Path, room: &str, name: &str, kind: &str, now: u64) -> Result<(String, Take), String> {
    let room = clean_name(room, "Call");
    let person = clean_name(name, "Someone");
    let kind = if kind == "mp4" { "mp4" } else { "webm" }.to_string();
    {
        let book = book();
        let open = book.takes.values().filter(|take| !take.done && now.saturating_sub(take.last_ms) < QUIET_MS).count();
        if open >= MAX_OPEN {
            return Err("Too many recordings are going at once.".into());
        }
    }
    fs::create_dir_all(root).map_err(|_| "Keel could not make the Recordings folder. Is the Files disk connected?".to_string())?;
    if free_bytes(root).is_some_and(|free| free < KEEP_FREE) {
        return Err("The Files disk is almost full, so Keel is not recording.".into());
    }
    let (folder, mut call) = call_for(root, &room, now)?;
    call.last_ms = now;
    write_json(&root.join(&folder).join(CALL_FILE), &call);
    let mut book = book();
    let taken: Vec<String> = book.takes.values().filter(|take| take.call == folder).map(|take| take.person.clone()).collect();
    let person = unique_person(&root.join(&folder), &person, &taken);
    let id = new_id();
    let take = Take {
        token: new_id(),
        call: folder.clone(),
        person,
        kind,
        offset_ms: now.saturating_sub(call.started_ms),
        last_ms: now,
        bytes: 0,
        done: false,
    };
    let dir = root.join(&folder).join(PARTS).join(&id);
    fs::create_dir_all(&dir).map_err(|_| "Keel could not start the recording.".to_string())?;
    write_json(&dir.join("take.json"), &take);
    remember_offset(root, &take);
    book.takes.insert(id.clone(), take.clone());
    Ok((id, take))
}

/// The folder for a room's call: the one still going (a recording in the
/// last 15 minutes), or a new "2026-11-18 19.04 Room".
fn call_for(root: &Path, room: &str, now: u64) -> Result<(String, Call), String> {
    if let Ok(entries) = fs::read_dir(root) {
        let mut recent: Option<(String, Call)> = None;
        for entry in entries.flatten() {
            let Some(call) = fs::read(entry.path().join(CALL_FILE)).ok().and_then(|b| serde_json::from_slice::<Call>(&b).ok()) else {
                continue;
            };
            if call.room == room && now.saturating_sub(call.last_ms) < SAME_CALL_MS {
                let name = entry.file_name().to_string_lossy().to_string();
                if recent.as_ref().is_none_or(|(_, best)| call.last_ms > best.last_ms) {
                    recent = Some((name, call));
                }
            }
        }
        if let Some(found) = recent {
            return Ok(found);
        }
    }
    let stamp = local_stamp(now);
    let mut folder = format!("{stamp} {room}");
    let mut n = 2;
    while root.join(&folder).exists() {
        folder = format!("{stamp} {room} {n}");
        n += 1;
    }
    fs::create_dir_all(root.join(&folder)).map_err(|_| "Keel could not make the call's folder.".to_string())?;
    Ok((folder, Call { room: room.into(), started_ms: now, last_ms: now, ..Call::default() }))
}

/// "2026-11-18 19.04", in this computer's time.
fn local_stamp(ms: u64) -> String {
    std::process::Command::new("date")
        .args(["-d", &format!("@{}", ms / 1000), "+%F %H.%M"])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| (ms / 1000).to_string())
}

/// A name that is safe as a file name: letters, digits, spaces, dots, dashes.
fn clean_name(text: &str, fallback: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|word| !word.chars().all(|c| c == '.'))
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned = cleaned.trim_matches('.').trim().to_string();
    let cleaned: String = cleaned.chars().take(60).collect();
    if cleaned.is_empty() || cleaned.eq_ignore_ascii_case(COMBINED.trim_end_matches(".mp4")) {
        fallback.into()
    } else {
        cleaned
    }
}

fn unique_person(folder: &Path, person: &str, taken: &[String]) -> String {
    let used = |name: &str| {
        taken.iter().any(|t| t.eq_ignore_ascii_case(name))
            || ["webm", "mp4"].iter().any(|ext| folder.join(format!("{name}.{ext}")).exists())
    };
    if !used(person) {
        return person.into();
    }
    (2..).map(|n| format!("{person} {n}")).find(|name| !used(name)).unwrap_or_else(|| person.into())
}

fn new_id() -> String {
    let mut bytes = [0u8; 16];
    if fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes)).is_err() {
        let seed = now_ms() ^ (std::process::id() as u64).rotate_left(32);
        bytes[..8].copy_from_slice(&seed.to_le_bytes());
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Ok(body) = serde_json::to_vec(value) {
        let tmp = path.with_extension("tmp");
        if fs::write(&tmp, body).is_ok() {
            let _ = fs::rename(&tmp, path);
        }
    }
}

fn free_bytes(path: &Path) -> Option<u64> {
    let out = std::process::Command::new("df").arg("-Pk").arg(path).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let kb: u64 = text.lines().nth(1)?.split_whitespace().nth(3)?.parse().ok()?;
    Some(kb * 1024)
}

/// The take for `id`, when the token matches.
fn take_for(id: &str, headers: &HeaderMap) -> Result<Take, Answer> {
    let token = headers.get("x-keel-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    let book = book();
    match book.takes.get(id) {
        Some(take) if !token.is_empty() && take.token == token => Ok(take.clone()),
        _ => Err(no(StatusCode::NOT_FOUND, "Keel does not know that recording.")),
    }
}

fn parts_dir(root: &Path, take: &Take, id: &str) -> PathBuf {
    root.join(&take.call).join(PARTS).join(id)
}

/// Which pieces Keel has, so a browser that lost its connection sends only
/// the rest.
async fn have(UrlPath(id): UrlPath<String>, headers: HeaderMap) -> Answer {
    let take = match take_for(&id, &headers) {
        Ok(take) => take,
        Err(answer) => return answer,
    };
    let pieces = pieces_in(&parts_dir(&recordings_dir(), &take, &id));
    (StatusCode::OK, Json(serde_json::json!({ "ok": true, "have": pieces.iter().map(|(seq, _)| seq).collect::<Vec<_>>(), "done": take.done })))
}

fn pieces_in(dir: &Path) -> Vec<(u32, PathBuf)> {
    let mut pieces: Vec<(u32, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let seq = name.strip_suffix(".piece")?.parse::<u32>().ok()?;
            Some((seq, entry.path()))
        })
        .collect();
    pieces.sort_by_key(|(seq, _)| *seq);
    pieces
}

async fn piece(UrlPath((id, seq)): UrlPath<(String, u32)>, headers: HeaderMap, body: Bytes) -> Answer {
    let take = match take_for(&id, &headers) {
        Ok(take) => take,
        Err(answer) => return answer,
    };
    if take.done {
        return (StatusCode::OK, Json(serde_json::json!({ "ok": true, "done": true })));
    }
    if body.len() > MAX_PIECE {
        return no(StatusCode::PAYLOAD_TOO_LARGE, "That piece is too large.");
    }
    if take.bytes + body.len() as u64 > MAX_PERSON {
        return no(StatusCode::PAYLOAD_TOO_LARGE, "This recording is as long as Keel keeps.");
    }
    let root = recordings_dir();
    let saved = tokio::task::spawn_blocking(move || -> Result<(), String> {
        if free_bytes(&root).is_some_and(|free| free < KEEP_FREE) {
            return Err("The Files disk is almost full, so Keel stopped recording.".into());
        }
        let dir = parts_dir(&root, &take, &id);
        let path = dir.join(format!("{seq:07}.piece"));
        let fresh = !path.exists();
        let tmp = dir.join(format!("{seq:07}.tmp"));
        let mut file = fs::File::create(&tmp).map_err(|_| "Keel could not keep that piece.".to_string())?;
        file.write_all(&body).map_err(|_| "Keel could not keep that piece.".to_string())?;
        fs::rename(&tmp, &path).map_err(|_| "Keel could not keep that piece.".to_string())?;
        let mut book = book();
        if let Some(kept) = book.takes.get_mut(&id) {
            kept.last_ms = now_ms();
            if fresh {
                kept.bytes += body.len() as u64;
            }
            write_json(&dir.join("take.json"), kept);
        }
        Ok(())
    })
    .await;
    match saved {
        Ok(Ok(())) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Ok(Err(message)) => no(StatusCode::INSUFFICIENT_STORAGE, &message),
        Err(_) => no(StatusCode::INTERNAL_SERVER_ERROR, "Keel could not keep that piece."),
    }
}

async fn done(UrlPath(id): UrlPath<String>, headers: HeaderMap) -> Answer {
    let take = match take_for(&id, &headers) {
        Ok(take) => take,
        Err(answer) => return answer,
    };
    let root = recordings_dir();
    let finished = tokio::task::spawn_blocking(move || finish(&root, &id, &take, now_ms())).await;
    match finished {
        Ok(Ok(file)) => (StatusCode::OK, Json(serde_json::json!({ "ok": true, "file": file }))),
        Ok(Err(message)) => no(StatusCode::INTERNAL_SERVER_ERROR, &message),
        Err(_) => no(StatusCode::INTERNAL_SERVER_ERROR, "Keel could not finish the recording."),
    }
}

/// Joins the pieces in order into "<person>.<kind>", tidies the file so it
/// seeks and shows its length, and starts the combined video when every
/// recording in the call is finished.
fn finish(root: &Path, id: &str, take: &Take, now: u64) -> Result<String, String> {
    if take.done {
        return Ok(format!("{}.{}", take.person, take.kind));
    }
    let dir = parts_dir(root, take, id);
    let folder = root.join(&take.call);
    let pieces = pieces_in(&dir);
    let name = format!("{}.{}", take.person, take.kind);
    let target = folder.join(&name);
    if !pieces.is_empty() {
        let joined = dir.join(format!("joined.{}", take.kind));
        {
            let mut out = fs::File::create(&joined).map_err(|_| "Keel could not join the recording.".to_string())?;
            for (_, path) in &pieces {
                let bytes = fs::read(path).map_err(|_| "Keel could not join the recording.".to_string())?;
                out.write_all(&bytes).map_err(|_| "Keel could not join the recording.".to_string())?;
            }
        }
        // A browser writes the file as it goes, without its length. A copy
        // through ffmpeg adds it, without changing the picture or sound.
        let tidied = dir.join(format!("tidied.{}", take.kind));
        let tidy_ok = ffmpeg()
            .map(|ffmpeg| {
                std::process::Command::new(ffmpeg)
                    .args(["-nostdin", "-loglevel", "error", "-y", "-fflags", "+genpts", "-i"])
                    .arg(&joined)
                    .args(["-c", "copy"])
                    .arg(&tidied)
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success())
            })
            .unwrap_or(false);
        let from = if tidy_ok { &tidied } else { &joined };
        fs::rename(from, &target).map_err(|_| "Keel could not keep the recording.".to_string())?;
    }
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir(folder.join(PARTS));
    let call_path = folder.join(CALL_FILE);
    if let Some(mut call) = fs::read(&call_path).ok().and_then(|b| serde_json::from_slice::<Call>(&b).ok()) {
        call.last_ms = now;
        write_json(&call_path, &call);
    }
    book().takes.remove(id);
    Ok(name)
}

/// Called about once a minute: a recording whose browser went away without
/// saying it was done (the tab closed, the computer slept) is finished with
/// what arrived, after 30 quiet minutes.
pub fn tidy() {
    let root = recordings_dir();
    let now = now_ms();
    let stale: Vec<(String, Take)> = book()
        .takes
        .iter()
        .filter(|(_, take)| !take.done && now.saturating_sub(take.last_ms) >= QUIET_MS)
        .map(|(id, take)| (id.clone(), take.clone()))
        .collect();
    for (id, take) in stale {
        let _ = finish(&root, &id, &take, now);
    }
    if let Some(folder) = settled_call(&root, now) {
        if !COMBINING.swap(true, std::sync::atomic::Ordering::SeqCst) {
            std::thread::spawn(move || {
                combine_call(&root, &folder);
                COMBINING.store(false, std::sync::atomic::Ordering::SeqCst);
            });
        }
    }
}

/// A call whose recordings are all finished, quiet for two minutes, and
/// whose combined video is missing or holds fewer people than the folder.
fn settled_call(root: &Path, now: u64) -> Option<String> {
    let open: Vec<String> = book().takes.values().filter(|t| !t.done).map(|t| t.call.clone()).collect();
    fs::read_dir(root).ok()?.flatten().find_map(|entry| {
        let folder = entry.file_name().to_string_lossy().to_string();
        let call: Call = fs::read(entry.path().join(CALL_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
        if open.contains(&folder) || now.saturating_sub(call.last_ms) < SETTLE_MS {
            return None;
        }
        let people = offsets_in(&entry.path(), &call).len();
        (people >= 2 && (call.people != people || call.combined == "making")).then_some(folder)
    })
}

fn ffmpeg() -> Option<PathBuf> {
    ["/usr/bin/ffmpeg", "/usr/local/bin/ffmpeg"].iter().map(PathBuf::from).find(|path| path.is_file())
}

fn has_audio(file: &Path) -> bool {
    std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "a", "-show_entries", "stream=index", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .map(|out| !String::from_utf8_lossy(&out.stdout).trim().is_empty())
        .unwrap_or(true)
}

/// Makes Everyone.mp4 from each person's file, placed in a grid, each
/// starting when that person started. Runs at the lowest priority: on a Pi
/// it takes longer than the call did.
fn combine_call(root: &Path, folder: &str) {
    let dir = root.join(folder);
    let call_path = dir.join(CALL_FILE);
    let mut call: Call = fs::read(&call_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let offsets = offsets_in(&dir, &call);
    if offsets.len() < 2 {
        return;
    }
    let people = offsets.len();
    let Some(ffmpeg) = ffmpeg() else {
        call.combined = "Keel needs ffmpeg to put everyone in one video. Open a video in Photos once, and Keel adds it.".into();
        write_json(&call_path, &call);
        return;
    };
    call.combined = "making".into();
    write_json(&call_path, &call);
    let inputs: Vec<(PathBuf, u64, bool)> = offsets.into_iter().map(|(path, offset)| {
        let audio = has_audio(&path);
        (path, offset, audio)
    }).collect();
    let partial = dir.join(".Everyone.making.mp4");
    let mut command = std::process::Command::new("nice");
    command.args(["-n", "19"]).arg(ffmpeg).args(["-nostdin", "-loglevel", "error", "-y"]);
    for (path, _, _) in &inputs {
        command.arg("-i").arg(path);
    }
    command
        .args(["-filter_complex", &combine_filter(&inputs.iter().map(|(_, o, a)| (*o, *a)).collect::<Vec<_>>())])
        .args(["-map", "[v]"]);
    if inputs.iter().any(|(_, _, audio)| *audio) {
        command.args(["-map", "[a]", "-c:a", "aac", "-b:a", "128k"]);
    }
    command
        .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "26", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(&partial);
    let ok = command.status().is_ok_and(|status| status.success());
    call.people = people;
    if ok && fs::rename(&partial, dir.join(COMBINED)).is_ok() {
        call.combined = "done".into();
    } else {
        let _ = fs::remove_file(&partial);
        call.combined = "Keel could not put everyone in one video. Each person's own file is in this folder.".into();
    }
    write_json(&call_path, &call);
}

/// Each person's finished file and when they started after the call began.
fn offsets_in(dir: &Path, _call: &Call) -> Vec<(PathBuf, u64)> {
    let takes = load_offsets(dir);
    let mut files: Vec<(PathBuf, u64)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            !name.starts_with('.') && name != COMBINED && matches!(path.extension().and_then(|e| e.to_str()), Some("webm" | "mp4"))
        })
        .map(|path| {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let offset = takes.get(&stem).copied().unwrap_or(0);
            (path, offset)
        })
        .collect();
    files.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    files.truncate(9);
    files
}

/// Start times are kept beside the files, so the combined video can place
/// everyone even after a restart.
fn load_offsets(dir: &Path) -> BTreeMap<String, u64> {
    fs::read(dir.join(".offsets.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// The ffmpeg filter for `inputs` (start offset in ms, has sound): each
/// picture scaled into a 640×360 cell, black until that person started,
/// the cells in a grid, and the sound of everyone together.
fn combine_filter(inputs: &[(u64, bool)]) -> String {
    let n = inputs.len();
    let columns = match n { 0..=1 => 1, 2..=4 => 2, _ => 3 };
    let mut parts = Vec::new();
    for (i, (offset, _)) in inputs.iter().enumerate() {
        parts.push(format!(
            "[{i}:v]setpts=PTS-STARTPTS,fps=25,scale=640:360:force_original_aspect_ratio=decrease,pad=640:360:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,tpad=start_duration={:.3}:color=black[v{i}]",
            *offset as f64 / 1000.0
        ));
    }
    let layout: Vec<String> = (0..n)
        .map(|i| format!("{}_{}", (i % columns) * 640, (i / columns) * 360))
        .collect();
    let cells: String = (0..n).map(|i| format!("[v{i}]")).collect();
    parts.push(format!("{cells}xstack=inputs={n}:layout={}:fill=black[v]", layout.join("|")));
    let sounds: Vec<usize> = inputs.iter().enumerate().filter(|(_, (_, audio))| *audio).map(|(i, _)| i).collect();
    if !sounds.is_empty() {
        for &i in &sounds {
            parts.push(format!("[{i}:a]aresample=48000,adelay={}:all=1[a{i}]", inputs[i].0));
        }
        let mixed: String = sounds.iter().map(|i| format!("[a{i}]")).collect();
        parts.push(format!("{mixed}amix=inputs={}:duration=longest:normalize=0[a]", sounds.len()));
    }
    parts.join(";")
}

/// Keeps each person's start time beside the call's files.
fn remember_offset(root: &Path, take: &Take) {
    let dir = root.join(&take.call);
    let mut offsets = load_offsets(&dir);
    offsets.insert(take.person.clone(), take.offset_ms);
    write_json(&dir.join(".offsets.json"), &offsets);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keel-rec-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_are_safe_file_names() {
        assert_eq!(clean_name("Ada Lovelace", "Someone"), "Ada Lovelace");
        assert_eq!(clean_name("../../etc/passwd", "Someone"), "etc passwd");
        assert_eq!(clean_name("  ", "Someone"), "Someone");
        assert_eq!(clean_name("Everyone", "Someone"), "Someone");
        assert_eq!(clean_name("Bob/<script>", "Someone"), "Bob script");
        assert!(clean_name(&"x".repeat(200), "Someone").len() <= 60);
    }

    #[test]
    fn a_call_keeps_one_file_per_person_and_joins_pieces_in_order() {
        let root = temp("call");
        let now = 1_795_000_000_000;
        let (id_a, ada) = begin(&root, "Family", "Ada", "webm", now).unwrap();
        let (id_b, bob) = begin(&root, "Family", "Ada", "webm", now + 4_000).unwrap();
        assert_eq!(ada.call, bob.call, "the same room within minutes is the same call");
        assert_eq!(bob.person, "Ada 2", "two people with one name get two files");
        assert_eq!(bob.offset_ms, 4_000);
        let dir = parts_dir(&root, &ada, &id_a);
        // Pieces arrive out of order after a dropped connection.
        fs::write(dir.join("0000001.piece"), b"world").unwrap();
        fs::write(dir.join("0000000.piece"), b"hello ").unwrap();
        let file = finish(&root, &id_a, &ada, now + 5_000).unwrap();
        assert_eq!(file, "Ada.webm");
        let kept = fs::read(root.join(&ada.call).join("Ada.webm")).unwrap();
        assert_eq!(kept, b"hello world", "not a real video, so ffmpeg leaves it as joined");
        assert!(!dir.exists(), "the pieces are gone once joined");
        let later = begin(&root, "Family", "Cy", "webm", now + SAME_CALL_MS + 60_000).unwrap();
        assert_ne!(later.1.call, ada.call, "a much later recording is a new call");
        let _ = id_b;
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn everyone_is_combined_only_when_the_call_has_settled() {
        let root = temp("settle");
        let now = 1_795_000_000_000;
        let (id_a, ada) = begin(&root, "Room", "Ada", "webm", now).unwrap();
        let (id_b, bob) = begin(&root, "Room", "Bob", "webm", now + 1_000).unwrap();
        fs::write(parts_dir(&root, &ada, &id_a).join("0000000.piece"), b"a").unwrap();
        finish(&root, &id_a, &ada, now + 2_000).unwrap();
        assert!(settled_call(&root, now + SETTLE_MS * 2).is_none(), "Bob is still recording");
        fs::write(parts_dir(&root, &bob, &id_b).join("0000000.piece"), b"b").unwrap();
        finish(&root, &id_b, &bob, now + 3_000).unwrap();
        assert!(settled_call(&root, now + 3_000).is_none(), "just finished: not yet");
        assert_eq!(settled_call(&root, now + 3_000 + SETTLE_MS + 1).as_deref(), Some(ada.call.as_str()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_combined_video_places_everyone_in_a_grid() {
        let two = combine_filter(&[(0, true), (2_500, true)]);
        assert!(two.contains("xstack=inputs=2:layout=0_0|640_0"));
        assert!(two.contains("tpad=start_duration=2.500"));
        assert!(two.contains("adelay=2500:all=1"));
        assert!(two.contains("amix=inputs=2"));
        let five = combine_filter(&[(0, true), (0, false), (0, true), (0, true), (0, true)]);
        assert!(five.contains("layout=0_0|640_0|1280_0|0_360|640_360"));
        assert!(five.contains("amix=inputs=4"), "a camera without sound is left out of the mix");
        assert!(!combine_filter(&[(0, false), (0, false)]).contains("amix"));
    }
}
