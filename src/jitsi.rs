//! Video is the upstream Jitsi meeting program, pinned, in containers.
//!
//! The meeting address is video.<the name you own>. The front door holds the
//! lock. The page itself stays on this computer. UDP 10000 carries the call.
//! Nothing here turns on a meeting timer. The call ends when people leave.
//! Close stops the programs. They stay on disk, and Open starts them again.
//! How many people a call holds, and how much memory the meeting programs
//! may take, come from this computer's memory: two people on a small one.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde::Serialize;

const PROGRESS_PATH: &str = "/run/keel/jitsi.json";
const INSTALLED: &str = "/var/lib/jitsi/installed";
const TAG: &str = "stable-11248-2";

static OPENING: AtomicBool = AtomicBool::new(false);

/// Video closes by itself after this long without a call, to give the
/// memory back. About 1 GB on a Pi.
pub const IDLE_MINUTES: u64 = 30;
const CLOSED_NOTE: &str = "/run/keel/jitsi-closed";
/// When a call was last seen, or Video was last found open, in seconds.
static LAST_CALL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Serialize)]
pub struct Status {
    pub ready: bool,
    pub address: String,
    pub message: String,
    pub state: String,
    pub percent: u8,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    state: String,
    percent: u8,
    message: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    ready: bool,
}

pub fn view() -> Status {
    if !cfg!(target_os = "linux") {
        return Status {
            ready: false,
            address: String::new(),
            message: "Video installs on the Keel computer.".into(),
            state: "unavailable".into(),
            percent: 0,
        };
    }
    if let Some(saved) = read_progress() {
        if saved.state == "running" && !OPENING.load(Ordering::SeqCst) {
            return idle();
        }
        if saved.state == "running" || saved.state == "error" || saved.ready {
            return Status {
                ready: saved.ready,
                address: saved.address,
                message: saved.message,
                state: saved.state,
                percent: saved.percent,
            };
        }
    }
    idle()
}

pub fn start() -> Result<Status, String> {
    if !cfg!(target_os = "linux") {
        return Err("Video installs on the Keel computer.".into());
    }
    crate::names::prepared()?;
    if !view().ready {
        crate::memory::room_for("Video", crate::memory::NEEDS_VIDEO)?;
    }
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = meeting_address(&owned)?;
    if OPENING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(view());
    }
    let _ = fs::remove_file(CLOSED_NOTE);
    LAST_CALL.store(0, Ordering::SeqCst);
    write_progress("running", 8, "Opening Video. This can take a while.", "", false);
    let host = address.trim_start_matches("https://").to_string();
    let script = setup_script(&host);
    std::thread::spawn(move || {
        write_progress("running", 36, "Opening Video.", "", false);
        if let Err(message) = crate::storage::host("exec /bin/bash -s", &[], Some(&script)) {
            write_progress("error", 0, &message, "", false);
            OPENING.store(false, Ordering::SeqCst);
            return;
        }
        write_progress("running", 92, "Writing the name.", "", false);
        match crate::names::publish("video", "keel-jitsi-web", 80) {
            Ok(waiting) => write_progress("done", 100, &ready_message(&address, waiting), &address, true),
            Err(message) => write_progress("error", 0, &message, "", false),
        }
        OPENING.store(false, Ordering::SeqCst);
    });
    Ok(view())
}

pub fn close() -> Result<Status, String> {
    if !cfg!(target_os = "linux") {
        return Err("Video installs on the Keel computer.".into());
    }
    if OPENING.load(Ordering::SeqCst) {
        return Err("Video is still opening. Try Close again in a moment.".into());
    }
    crate::storage::host("exec /bin/bash -s", &[], Some(close_script()))
        .map_err(|_| "Keel could not close Video. Try Close again.".to_string())?;
    let _ = fs::remove_file(PROGRESS_PATH);
    Ok(idle())
}

/// Called about once a minute. Closes Video when it is open and nobody has
/// been in a call for IDLE_MINUTES. When Keel cannot tell, it counts as a
/// call: a call is never cut off by a guess.
pub fn close_if_quiet() {
    if !cfg!(target_os = "linux") || OPENING.load(Ordering::SeqCst) || !view().ready {
        LAST_CALL.store(0, Ordering::SeqCst);
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last = LAST_CALL.load(Ordering::SeqCst);
    if last == 0 {
        // Just opened, or Keel just started: the quiet time starts now.
        LAST_CALL.store(now, Ordering::SeqCst);
        return;
    }
    // Asking costs a little, so ask only every few minutes until the time is near.
    if now.saturating_sub(last) < (IDLE_MINUTES - 5) * 60 && now % 300 >= 60 {
        return;
    }
    let stats = crate::storage::host_all("exec /bin/bash -s", &[], Some(STATS_SCRIPT)).unwrap_or_default();
    if busy_from_stats(&stats).unwrap_or(true) {
        LAST_CALL.store(now, Ordering::SeqCst);
        return;
    }
    if now.saturating_sub(last) < IDLE_MINUTES * 60 {
        return;
    }
    if close().is_ok() {
        let _ = fs::write(CLOSED_NOTE, format!("{now}\n"));
        LAST_CALL.store(0, Ordering::SeqCst);
        eprintln!("keel: Video closed by itself after {IDLE_MINUTES} minutes without a call");
    }
}

/// The meeting organiser's and the video router's own counts. Prints the
/// first answer, or {} when neither answers.
const STATS_SCRIPT: &str = r#"set -uo pipefail
for pair in keel-jitsi-jicofo:8888/stats keel-jitsi-jvb:8080/colibri/stats; do
  name=${pair%%:*}
  rest=${pair#*:}
  out=$(podman exec "$name" curl -fsS --max-time 4 "http://127.0.0.1:$rest" 2>/dev/null) && { printf '%s\n' "$out"; exit 0; }
  ip=$(podman inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$name" 2>/dev/null)
  [ -n "$ip" ] || continue
  out=$(curl -fsS --max-time 4 "http://$ip:$rest" 2>/dev/null) && { printf '%s\n' "$out"; exit 0; }
done
echo '{}'
"#;

/// Whether a call is going on, from Jicofo's /stats or the video router's
/// /colibri/stats: any conference or participant, including two people
/// talking directly (p2p). None when the answer says nothing.
fn busy_from_stats(text: &str) -> Option<bool> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let mut seen = false;
    let mut busy = false;
    for key in ["conferences", "participants", "p2p_conferences"] {
        if let Some(count) = value.get(key).and_then(serde_json::Value::as_u64) {
            seen = true;
            busy |= count > 0;
        }
    }
    seen.then_some(busy)
}

pub fn close_script() -> &'static str {
    "systemctl disable --now keel-jitsi.service >/dev/null 2>&1 || true\npodman rm -f keel-jitsi-web keel-jitsi-jvb keel-jitsi-jicofo keel-jitsi-prosody >/dev/null 2>&1 || true\n"
}

/// How large a call this computer can carry: the people in one call, and
/// the most memory the video router and the meeting organiser may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallSize {
    pub people: u32,
    pub bridge_mb: u32,
    pub focus_mb: u32,
}

pub fn call_size(memory_bytes: u64) -> CallSize {
    const GB: u64 = 1024 * 1024 * 1024;
    if memory_bytes < 6 * GB {
        CallSize { people: 2, bridge_mb: 512, focus_mb: 256 }
    } else if memory_bytes < 12 * GB {
        CallSize { people: 5, bridge_mb: 1024, focus_mb: 512 }
    } else {
        CallSize { people: 10, bridge_mb: 2048, focus_mb: 1024 }
    }
}

/// The call size for this computer, read from its memory.
pub fn this_call_size() -> CallSize {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    call_size(sys.total_memory())
}

fn people_line(size: CallSize) -> String {
    format!("Calls of up to {} people on this computer.", size.people)
}

pub fn meeting_address(owned: &str) -> Result<String, String> {
    crate::owned::app_address(owned, "video")
}

fn ready_message(address: &str, waiting: bool) -> String {
    format!(
        "The meeting is {address}. {} The call keeps going until people leave. For a call from outside the house, the router also sends UDP 10000 to this computer.{} You do not type addresses.",
        people_line(this_call_size()),
        crate::names::lock_note(waiting)
    )
}

fn idle() -> Status {
    let installed = Path::new(INSTALLED).is_file();
    let by_itself = if Path::new(CLOSED_NOTE).is_file() {
        format!("Video closed by itself after {IDLE_MINUTES} minutes without a call, to give the memory back. ")
    } else {
        String::new()
    };
    Status {
        ready: false,
        address: String::new(),
        message: if installed {
            format!("{by_itself}Video is on this computer. Press Open when you want a call. {}", people_line(this_call_size()))
        } else {
            format!(
                "Press Open when you want a call. Keel makes the video name and asks for the lock. The first time downloads the meeting program. {}",
                people_line(this_call_size())
            )
        },
        state: "idle".into(),
        percent: 0,
    }
}

fn read_progress() -> Option<Saved> {
    let bytes = fs::read(PROGRESS_PATH).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_progress(state: &str, percent: u8, message: &str, address: &str, ready: bool) {
    let saved = Saved {
        state: state.into(),
        percent,
        message: message.into(),
        address: address.into(),
        ready,
    };
    let Ok(body) = serde_json::to_vec(&saved) else {
        return;
    };
    let _ = fs::create_dir_all("/run/keel");
    let tmp = format!("{PROGRESS_PATH}.tmp");
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, PROGRESS_PATH);
    }
}

pub fn setup_script(name: &str) -> String {
    setup_script_for(name, this_call_size())
}

fn setup_script_for(name: &str, size: CallSize) -> String {
    let mut script = include_str!("jitsi-setup.sh").to_string();
    script = script.replace("__KEEL_NAME__", name);
    script = script.replace("__KEEL_TAG__", TAG);
    script = script.replace("__KEEL_PEOPLE__", &size.people.to_string());
    script = script.replace("__KEEL_BRIDGE__", &size.bridge_mb.to_string());
    script = script.replace("__KEEL_FOCUS__", &size.focus_mb.to_string());
    script
}

#[cfg(test)]
mod tests {
    use super::{meeting_address, setup_script};

    #[test]
    fn a_quiet_call_is_told_from_a_busy_one() {
        assert_eq!(super::busy_from_stats(r#"{"conferences":0,"participants":0}"#), Some(false));
        assert_eq!(super::busy_from_stats(r#"{"conferences":1,"participants":2}"#), Some(true));
        // Two people talking directly still count.
        assert_eq!(super::busy_from_stats(r#"{"conferences":0,"participants":0,"p2p_conferences":1}"#), Some(true));
        // Nothing to tell by: the caller treats it as a call.
        assert_eq!(super::busy_from_stats("{}"), None);
        assert_eq!(super::busy_from_stats("not json"), None);
        assert!(super::STATS_SCRIPT.contains("8888/stats"));
        assert!(super::STATS_SCRIPT.contains("8080/colibri/stats"));
    }

    #[test]
    fn the_meeting_address_is_the_bought_name() {
        assert_eq!(
            meeting_address("klaingar.com").as_deref(),
            Ok("https://video.klaingar.com")
        );
        assert!(meeting_address("").is_err());
        assert!(meeting_address("https://klaingar.com").is_err());
        assert!(meeting_address("keel.example.ts.net").is_err());
    }

    #[test]
    fn the_setup_uses_the_pinned_program_and_no_timer() {
        let script = setup_script("video.klaingar.com");
        assert!(script.contains("ghcr.io/jitsi/web:stable-11248-2"));
        assert!(script.contains("ghcr.io/jitsi/prosody:stable-11248-2"));
        assert!(script.contains("ghcr.io/jitsi/jicofo:stable-11248-2"));
        assert!(script.contains("ghcr.io/jitsi/jvb:stable-11248-2"));
        assert!(script.contains("name='video.klaingar.com'"));
        assert!(script.contains("PUBLIC_URL=https://$name"));
        assert!(script.contains("DISABLE_HTTPS=1"));
        assert!(script.contains("JVB_ADVERTISE_IPS=$house"));
        assert!(script.contains("-p 127.0.0.1:8080:80"));
        // The meeting programs need a writable system folder, as upstream runs them.
        assert!(!script.contains("--read-only"));
        assert!(script.contains("/keel-recording/recorder.js"));
        assert!(script.contains("config.disableDeepLinking = true;"));
        assert!(script.contains("podman logs --tail 80 keel-jitsi-web"));
        assert!(script.contains("-p 10000:10000/udp"));
        assert!(!script.contains("8443"));
        assert!(!script.contains("tailscale cert"));
        assert!(script.contains("JVB_DISABLE_STUN=1"));
        assert!(script.contains("ENABLE_P2P=1"));
        assert!(script.contains("PROSODY_RESERVATION_ENABLED=0"));
        assert!(!script.contains("time_restricted"));
        assert!(!script.contains("conference_max_minutes"));
        assert!(!script.contains("meet.jit.si"));
    }

    #[test]
    fn the_call_fits_the_computer() {
        const GB: u64 = 1024 * 1024 * 1024;
        let small = super::call_size(4 * GB);
        assert_eq!(small, super::CallSize { people: 2, bridge_mb: 512, focus_mb: 256 });
        assert_eq!(super::call_size(8 * GB).people, 5);
        assert_eq!(super::call_size(16 * GB).people, 10);
        let script = super::setup_script_for("video.example.com", small);
        assert!(script.contains("MAX_PARTICIPANTS=2\n"));
        assert!(script.contains("VIDEOBRIDGE_MAX_MEMORY=512m\n"));
        assert!(script.contains("JICOFO_MAX_MEMORY=256m\n"));
        assert!(!script.contains("__KEEL_"));
    }

    #[test]
    fn close_stops_the_call_and_keeps_the_program() {
        let script = super::close_script();
        assert!(script.contains("podman rm -f keel-jitsi-web keel-jitsi-jvb keel-jitsi-jicofo keel-jitsi-prosody"));
        assert!(!script.contains("/var/lib/jitsi"));
    }
}
