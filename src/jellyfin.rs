//! Watch is films, shows, and music already in Media.
//! A small computer is refused. The address is watch.<the name you own>.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use sysinfo::System;

const PROGRESS_PATH: &str = "/run/keel/jellyfin.json";
const INSTALLED: &str = "/var/lib/jellyfin/installed";
/// Six gigabytes. A 4 GB computer is refused. An 8 GB computer is offered Jellyfin.
pub const MIN_BYTES: u64 = 6 * 1024 * 1024 * 1024;

static OPENING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
pub struct Status {
    pub ready: bool,
    pub address: String,
    pub message: String,
    pub state: String,
    pub percent: u8,
    pub allowed: bool,
}

#[derive(serde::Deserialize)]
struct Saved {
    state: String,
    percent: u8,
    message: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    ready: bool,
}

pub fn fits(total: u64) -> bool {
    total >= MIN_BYTES
}

pub fn view() -> Status {
    if !machine_fits() {
        return refused();
    }
    if !cfg!(target_os = "linux") {
        return Status {
            ready: false,
            address: String::new(),
            message: "Watch installs on the Keel computer.".into(),
            state: "unavailable".into(),
            percent: 0,
            allowed: true,
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
                allowed: true,
            };
        }
    }
    idle()
}

pub fn start() -> Result<Status, String> {
    if !machine_fits() {
        return Err("Watch needs a larger computer.".into());
    }
    if !cfg!(target_os = "linux") {
        return Err("Watch installs on the Keel computer.".into());
    }
    crate::names::prepared()?;
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = crate::owned::app_address(&owned, "watch")?;
    if OPENING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(view());
    }
    write_progress("running", 8, "Opening Watch. This can take a while.", "", false);
    std::thread::spawn(move || {
        write_progress(
            "running",
            36,
            "Downloading Watch. Films, shows, and music stay in Media.",
            "",
            false,
        );
        if let Err(message) = crate::storage::host("exec /bin/bash -s", &[], Some(include_str!("jellyfin-setup.sh")))
        {
            write_progress("error", 0, &message, "", false);
            OPENING.store(false, Ordering::SeqCst);
            return;
        }
        write_progress("running", 92, "Writing the name.", "", false);
        match crate::names::publish("watch", "keel-jellyfin", 8096) {
            Ok(waiting) => write_progress("done", 100, &ready_message(&address, waiting), &address, true),
            Err(message) => write_progress("error", 0, &message, "", false),
        }
        OPENING.store(false, Ordering::SeqCst);
    });
    Ok(view())
}

fn machine_fits() -> bool {
    let mut sys = System::new();
    sys.refresh_memory();
    fits(sys.total_memory())
}

fn refused() -> Status {
    Status {
        ready: false,
        address: String::new(),
        message: "Watch needs a larger computer.".into(),
        state: "refused".into(),
        percent: 0,
        allowed: false,
    }
}

fn idle() -> Status {
    let installed = Path::new(INSTALLED).is_file();
    Status {
        ready: false,
        address: String::new(),
        message: if installed {
            "Watch is on this computer. Press Open to start it.".into()
        } else {
            "Films, shows, and music already in Media. Press Open. The first time downloads the program.".into()
        },
        state: "idle".into(),
        percent: 0,
        allowed: true,
    }
}

/// Open shows Watch on Keel's own address and signs in, so the card needs no
/// address or setup step.
fn ready_message(_address: &str, _waiting: bool) -> String {
    "Press Open. Films, shows, and music in Media show up there.".into()
}

fn read_progress() -> Option<Saved> {
    let bytes = fs::read(PROGRESS_PATH).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_progress(state: &str, percent: u8, message: &str, address: &str, ready: bool) {
    let body = format!(
        "{{\"state\":\"{state}\",\"percent\":{percent},\"message\":{},\"address\":{},\"ready\":{ready}}}",
        serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(address).unwrap_or_else(|_| "\"\"".into()),
    );
    let _ = fs::create_dir_all("/run/keel");
    let tmp = format!("{PROGRESS_PATH}.tmp");
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, PROGRESS_PATH);
    }
}

#[cfg(test)]
mod tests {
    use super::{fits, MIN_BYTES};

    #[test]
    fn a_small_computer_is_refused_jellyfin() {
        assert!(!fits(0));
        assert!(!fits(4 * 1024 * 1024 * 1024));
        assert!(!fits(MIN_BYTES - 1));
        assert!(fits(MIN_BYTES));
        assert!(fits(8 * 1024 * 1024 * 1024));
        let script = include_str!("jellyfin-setup.sh");
        assert!(!script.contains('\r'));
        assert!(script.contains("docker.io/jellyfin/jellyfin:10.10.7"));
        assert!(script.contains("/srv/keel/files/Media"));
        assert!(script.contains("-p 127.0.0.1:8096:8096"));
        assert!(script.contains("--memory=2g"));
        assert!(!script.contains("0.0.0.0"));
        assert!(!script.contains("9478"));
    }
}
