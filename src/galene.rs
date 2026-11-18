//! Light Video: Galène, shipped in the Keel package, run as keel-galene.
//! Same name and card as the full Video (Jitsi); Settings picks which.
//! The meeting link is video.<name>/group/call/ and any room under it, such
//! as video.<name>/group/call/family/, opens when first visited.

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::jitsi::Status;

const PROGRESS_PATH: &str = "/run/keel/galene.json";
static OPENING: AtomicBool = AtomicBool::new(false);

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
        return Status { ready: false, address: String::new(), message: "Video installs on the Keel computer.".into(), state: "unavailable".into(), percent: 0 };
    }
    if let Some(saved) = read_progress() {
        if saved.state == "running" && !OPENING.load(Ordering::SeqCst) {
            return idle();
        }
        if saved.ready && !running() {
            return idle();
        }
        if saved.state == "running" || saved.state == "error" || saved.ready {
            return Status { ready: saved.ready, address: saved.address, message: saved.message, state: saved.state, percent: saved.percent };
        }
    }
    idle()
}

fn running() -> bool {
    std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", "keel-galene.service"])
        .status()
        .is_ok_and(|s| s.success())
}

fn idle() -> Status {
    Status {
        ready: false,
        address: String::new(),
        message: format!(
            "Light Video (Galène). Press Open when you want a call. Calls of up to {} people on this computer, recording included.",
            people()
        ),
        state: "idle".into(),
        percent: 0,
    }
}

/// People in one call: the light program carries more than Jitsi on the
/// same computer.
fn people() -> u32 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    people_for(sys.total_memory())
}

fn people_for(memory_bytes: u64) -> u32 {
    const GB: u64 = 1024 * 1024 * 1024;
    match memory_bytes {
        m if m < 2 * GB => 4,
        m if m < 6 * GB => 8,
        _ => 16,
    }
}

pub fn meeting_link(address: &str) -> String {
    format!("{}/group/call/", address.trim_end_matches('/'))
}

pub fn start() -> Result<Status, String> {
    if !cfg!(target_os = "linux") {
        return Err("Video installs on the Keel computer.".into());
    }
    crate::names::prepared()?;
    if !view().ready {
        crate::memory::room_for("Video", 150)?;
    }
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = crate::jitsi::meeting_address(&owned)?;
    if OPENING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Ok(view());
    }
    write_progress("running", 8, "Opening Video.", "", false);
    let host = address.trim_start_matches("https://").to_string();
    let script = setup_script(&host, people());
    std::thread::spawn(move || {
        if let Err(message) = crate::storage::host("exec /bin/bash -s", &[], Some(&script)) {
            write_progress("error", 0, &message, "", false);
            OPENING.store(false, Ordering::SeqCst);
            return;
        }
        write_progress("running", 92, "Writing the name.", "", false);
        let link = meeting_link(&address);
        match crate::names::publish("video", "keel-galene", 8444) {
            Ok(waiting) => write_progress(
                "done",
                100,
                &format!(
                    "The meeting is {link}. Send that link; anyone who opens it joins with a name they type. A room of its own is {link}family/ or any other word. Press Record call in a call to keep each person's video in Files.{}",
                    crate::names::lock_note(waiting)
                ),
                &link,
                true,
            ),
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
    crate::storage::host("exec /bin/bash -s", &[], Some("systemctl disable --now keel-galene.service >/dev/null 2>&1 || true\n"))
        .map_err(|_| "Keel could not close Video. Try Close again.".to_string())?;
    let _ = fs::remove_file(PROGRESS_PATH);
    Ok(idle())
}

pub fn setup_script(host: &str, people: u32) -> String {
    include_str!("galene-setup.sh").replace("__KEEL_NAME__", host).replace("__KEEL_PEOPLE__", &people.to_string())
}

fn read_progress() -> Option<Saved> {
    serde_json::from_slice(&fs::read(PROGRESS_PATH).ok()?).ok()
}

fn write_progress(state: &str, percent: u8, message: &str, address: &str, ready: bool) {
    let saved = Saved { state: state.into(), percent, message: message.into(), address: address.into(), ready };
    let Ok(body) = serde_json::to_vec(&saved) else {
        return;
    };
    let _ = fs::create_dir_all("/run/keel");
    let tmp = format!("{PROGRESS_PATH}.tmp");
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, PROGRESS_PATH);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_light_video_runs_galene_behind_the_door_with_a_ceiling() {
        let script = super::setup_script("video.klaingar.com", 4);
        assert!(script.contains("name='video.klaingar.com'"));
        assert!(script.contains("-insecure -http 127.0.0.1:8444"));
        assert!(script.contains("-udp-range 10000-10019"));
        assert!(script.contains(r#""proxyURL": "https://$name/""#));
        assert!(script.contains(r#""wildcard-user": { "password": { "type": "wildcard" }, "permissions": "present" }"#));
        assert!(script.contains(r#""allow-recording": true"#));
        assert!(script.contains(r#""max-clients": $people"#));
        assert!(script.contains("MemoryMax=300M"));
        assert!(script.contains("chmod 0755 \"$bin\""));
        assert!(script.contains("podman rm -f keel-jitsi-web"));
        assert!(!script.contains("__KEEL_"));
        assert!(!script.contains("9478"));
        assert!(!script.contains('\r'));
    }

    #[test]
    fn the_link_is_the_call_group() {
        assert_eq!(super::meeting_link("https://video.klaingar.com"), "https://video.klaingar.com/group/call/");
        assert_eq!(super::people_for(914 * 1024 * 1024), 4);
    }
}
