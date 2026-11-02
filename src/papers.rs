//! Papers files documents at papers.<the name you own>.
//!
//! Papers you add are stored in Documents. Keel does not watch that folder,
//! so files already there stay put. The program listens on this computer only.
//! The front door sends the name in.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde::Serialize;

const PROGRESS_PATH: &str = "/run/keel/papers.json";
const INSTALLED: &str = "/var/lib/papra/installed";

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
            message: "Papers install on the Keel computer.".into(),
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
        return Err("Papers install on the Keel computer.".into());
    }
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = crate::owned::app_address(&owned, "papers")?;
    if OPENING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(view());
    }
    write_progress("running", 2, "Opening Names.", "", false);
    let script = setup_script(&address.trim_start_matches("https://"));
    std::thread::spawn(move || {
        if let Err(message) = crate::names::ensure() {
            write_progress("error", 0, &message, "", false);
            OPENING.store(false, Ordering::SeqCst);
            return;
        }
        write_progress("running", 36, "Opening Papers.", "", false);
        let result = crate::storage::host_text("exec /bin/bash -s", &[], Some(&script));
        match result {
            Ok(line) if line == "ready" => {
                write_progress("running", 92, "Writing the name.", "", false);
                match crate::names::publish("papers", "keel-papers", 1221) {
                    Ok(waiting) => write_progress(
                        "done",
                        100,
                        &ready_message(&address, waiting),
                        &address,
                        true,
                    ),
                    Err(message) => write_progress("error", 0, &message, "", false),
                }
            }
            Ok(_) => write_progress(
                "error",
                0,
                "Keel could not tell whether Papers is ready.",
                "",
                false,
            ),
            Err(message) => write_progress("error", 0, &message, "", false),
        }
        OPENING.store(false, Ordering::SeqCst);
    });
    Ok(view())
}

fn ready_message(address: &str, waiting: bool) -> String {
    format!(
        "Papers are at {address}. Create the account for this house. Papers you add are stored in Documents. Documents stays the files.{} You do not type addresses.",
        crate::names::lock_note(waiting)
    )
}

fn idle() -> Status {
    let installed = Path::new(INSTALLED).is_file();
    Status {
        ready: false,
        address: String::new(),
        message: if installed {
            "Papers are on this computer. Press Open to start them.".into()
        } else {
            "Press Open. Keel makes the papers name and asks for the lock. The first time downloads the program.".into()
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

pub fn setup_script(host: &str) -> String {
    include_str!("papers-setup.sh").replace("__KEEL_HOST__", host)
}

#[cfg(test)]
mod tests {
    use super::setup_script;

    #[test]
    fn papers_use_the_pinned_program_and_keep_documents() {
        let script = setup_script("papers.klaingar.com");
        assert!(script.contains("host='papers.klaingar.com'"));
        assert!(script.contains("ghcr.io/papra-hq/papra:26.7.1-root"));
        assert!(script.contains("APP_BASE_URL=\"https://$host\""));
        assert!(script.contains("DOCUMENT_STORAGE_FILESYSTEM_ROOT=/documents"));
        assert!(script.contains("docs=/srv/keel/files/Documents"));
        assert!(script.contains("\"$docs:/documents\""));
        assert!(script.contains("-p 127.0.0.1:1221:1221"));
        assert!(script.contains("chmod 0600 \"$cfg/auth.secret\""));
        assert!(!script.contains("INGESTION_FOLDER"));
        assert!(!script.contains("0.0.0.0"));
        assert!(!script.contains("-p 1221:1221"));
        assert!(!script.contains("9478"));
    }
}
