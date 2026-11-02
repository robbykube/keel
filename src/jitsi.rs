//! Video is the upstream Jitsi meeting program, pinned, in containers.
//!
//! The meeting address is video.<the name you own>. The front door holds the
//! lock. The page itself stays on this computer. UDP 10000 carries the call.
//! Nothing here turns on a meeting timer. The call ends when people leave.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde::Serialize;

const PROGRESS_PATH: &str = "/run/keel/jitsi.json";
const INSTALLED: &str = "/var/lib/jitsi/installed";
const TAG: &str = "stable-11248-2";

static OPENING: AtomicBool = AtomicBool::new(false);

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
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = meeting_address(&owned)?;
    if OPENING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(view());
    }
    write_progress("running", 2, "Opening Names.", "", false);
    let host = address.trim_start_matches("https://").to_string();
    let script = setup_script(&host);
    std::thread::spawn(move || {
        if let Err(message) = crate::names::ensure() {
            write_progress("error", 0, &message, "", false);
            OPENING.store(false, Ordering::SeqCst);
            return;
        }
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

pub fn meeting_address(owned: &str) -> Result<String, String> {
    crate::owned::app_address(owned, "video")
}

fn ready_message(address: &str, waiting: bool) -> String {
    format!(
        "The meeting is {address}. The call keeps going until people leave. For a call from outside the house, the router also sends UDP 10000 to this computer.{} You do not type addresses.",
        crate::names::lock_note(waiting)
    )
}

fn idle() -> Status {
    let installed = Path::new(INSTALLED).is_file();
    Status {
        ready: false,
        address: String::new(),
        message: if installed {
            "Video is on this computer. Press Open when you want a call.".into()
        } else {
            "Press Open when you want a call. Keel makes the video name and asks for the lock. The first time downloads the meeting program.".into()
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
    let mut script = include_str!("jitsi-setup.sh").to_string();
    script = script.replace("__KEEL_NAME__", name);
    script = script.replace("__KEEL_TAG__", TAG);
    script
}

#[cfg(test)]
mod tests {
    use super::{meeting_address, setup_script};

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
}
