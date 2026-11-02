//! Photos shows the pictures already in Media.
//!
//! The address is photos.<the name you own>. Keel writes that name and asks
//! the front door for the lock. The person does not match it to an address.
//! The program listens on this computer only. The front door sends the name in.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde::Serialize;

const PROGRESS_PATH: &str = "/run/keel/photos.json";
const INSTALLED: &str = "/var/lib/immich/installed";

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
            message: "Photos install on the Keel computer.".into(),
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
        return Err("Photos install on the Keel computer.".into());
    }
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = crate::owned::photos_address(&owned)?;
    if OPENING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(view());
    }
    write_progress("running", 2, "Opening Names.", "", false);
    let script = setup_script(&owned);
    std::thread::spawn(move || {
        if let Err(message) = crate::names::ensure() {
            write_progress("error", 0, &message, "", false);
            OPENING.store(false, Ordering::SeqCst);
            return;
        }
        write_progress("running", 36, "Opening Photos.", "", false);
        let result = crate::storage::host_text("exec /bin/bash -s", &[], Some(&script));
        match result {
            Ok(line) => match outcome(&line) {
                Some((email, password, included)) => {
                    write_progress("running", 92, "Writing the name.", "", false);
                    match crate::names::publish("photos", "keel-photos-server", 2283) {
                        Ok(waiting) => write_progress(
                            "done",
                            100,
                            &ready_message(&address, waiting, &email, &password, included),
                            &address,
                            true,
                        ),
                        Err(message) => write_progress("error", 0, &message, "", false),
                    }
                }
                None => write_progress(
                    "error",
                    0,
                    "Keel could not tell whether Photos is ready.",
                    "",
                    false,
                ),
            },
            Err(message) => write_progress("error", 0, &message, "", false),
        }
        OPENING.store(false, Ordering::SeqCst);
    });
    Ok(view())
}

fn outcome(line: &str) -> Option<(String, String, bool)> {
    let mut parts = line.split('|');
    let email = parts.next()?.to_string();
    let password = parts.next()?.to_string();
    let included = parts.next()?;
    if parts.next().is_some() || email.is_empty() || password.is_empty() {
        return None;
    }
    if !email.starts_with("photos@") || email.contains(' ') {
        return None;
    }
    Some((email, password, included == "yes"))
}

fn ready_message(address: &str, waiting: bool, email: &str, password: &str, included: bool) -> String {
    let pictures = if included {
        "Pictures already in Media show there."
    } else {
        "Pictures stay in Media. Press Open again in a minute if they are not listed yet."
    };
    format!(
        "Photos are at {address}. Sign in as {email}. The password is {password}. {pictures}{} You do not type addresses.",
        crate::names::lock_note(waiting)
    )
}

fn idle() -> Status {
    let installed = Path::new(INSTALLED).is_file();
    Status {
        ready: false,
        address: String::new(),
        message: if installed {
            "Photos are on this computer. Press Open to start them.".into()
        } else {
            "Press Open. Keel makes the photos name and asks for the lock. The first time downloads the program.".into()
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

pub fn setup_script(owned: &str) -> String {
    include_str!("photos-setup.sh").replace("__KEEL_OWNED__", owned)
}

#[cfg(test)]
mod tests {
    use super::{outcome, setup_script};

    #[test]
    fn the_result_line_carries_the_account_without_an_address() {
        let parsed = outcome("photos@klaingar.com|abc123|yes").unwrap();
        assert_eq!(parsed.0, "photos@klaingar.com");
        assert_eq!(parsed.1, "abc123");
        assert!(parsed.2);
        assert!(outcome("photos@klaingar.com|abc123|no").is_some());
        assert!(outcome("admin@klaingar.com|abc|yes").is_none());
        assert!(outcome("waiting|photos@klaingar.com|abc123|yes").is_none());
        assert!(outcome("192.168.0.127").is_none());
    }

    #[test]
    fn photos_use_the_pinned_program_and_the_media_folder() {
        let script = setup_script("klaingar.com");
        assert!(script.contains("owned='klaingar.com'"));
        assert!(script.contains("ghcr.io/immich-app/immich-server:v3.2.2"));
        assert!(script.contains("ghcr.io/immich-app/immich-machine-learning:v3.2.2"));
        assert!(script.contains("docker.io/valkey/valkey:9@sha256:70739f85ad2ee01a726a965584a0f94895f01b0c60b3cc8b0aeef11eaa6888cf"));
        assert!(script.contains("photos=/srv/keel/files/Media/Photos"));
        assert!(script.contains("\"$photos:/photos:ro\""));
        assert!(script.contains("-p 127.0.0.1:2283:2283"));
        assert!(!script.contains("/api/nginx/proxy-hosts"));
        assert!(!script.contains("0.0.0.0"));
        assert!(!script.contains("-p 2283:2283"));
        assert!(!script.contains("9478"));
    }
}
