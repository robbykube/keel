//! Mail is Thunderbird on this computer, at mail.<the name you own>.
//!
//! The profile stays on this computer. Add Gmail or another account in the
//! page. Mail you move here stays here. The page listens on this computer
//! only. The front door sends the name in.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde::Serialize;

const PROGRESS_PATH: &str = "/run/keel/mail.json";
const INSTALLED: &str = "/var/lib/thunderbird/installed";

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
            message: "Mail installs on the Keel computer.".into(),
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
        return Err("Mail installs on the Keel computer.".into());
    }
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = crate::owned::app_address(&owned, "mail")?;
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
        write_progress("running", 36, "Opening Mail.", "", false);
        let result = crate::storage::host_text("exec /bin/bash -s", &[], Some(&script));
        match result {
            Ok(line) => match page_password(&line) {
                Some(password) => {
                    write_progress("running", 92, "Writing the name.", "", false);
                    match crate::names::publish("mail", "keel-mail", 3000) {
                        Ok(waiting) => write_progress(
                            "done",
                            100,
                            &ready_message(&address, &password, waiting),
                            &address,
                            true,
                        ),
                        Err(message) => write_progress("error", 0, &message, "", false),
                    }
                }
                None => write_progress(
                    "error",
                    0,
                    "Keel could not tell whether Mail is ready.",
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

fn page_password(line: &str) -> Option<String> {
    let (word, password) = line.split_once('|')?;
    if word != "ready" || password.is_empty() || password.contains(' ') || password.contains('|') {
        return None;
    }
    Some(password.to_string())
}

fn ready_message(address: &str, password: &str, waiting: bool) -> String {
    format!(
        "Mail is at {address}. Sign in as mail. The password is {password}. Add Gmail or another account there. Mail you move into this computer stays here.{} You do not type addresses.",
        crate::names::lock_note(waiting)
    )
}

fn idle() -> Status {
    let installed = Path::new(INSTALLED).is_file();
    Status {
        ready: false,
        address: String::new(),
        message: if installed {
            "Mail is on this computer. Press Open to start it.".into()
        } else {
            "Press Open. Keel makes the mail name and asks for the lock. The first time downloads the program.".into()
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
    include_str!("mail-setup.sh").replace("__KEEL_HOST__", host)
}

#[cfg(test)]
mod tests {
    use super::{page_password, setup_script};

    #[test]
    fn the_result_line_is_the_page_password() {
        assert_eq!(page_password("ready|abc123").as_deref(), Some("abc123"));
        assert!(page_password("ready|").is_none());
        assert!(page_password("ready|has space").is_none());
        assert!(page_password("waiting|abc").is_none());
    }

    #[test]
    fn mail_uses_the_pinned_program_and_keeps_the_profile_here() {
        let script = setup_script("mail.klaingar.com");
        assert!(script.contains("host='mail.klaingar.com'"));
        assert!(script.contains("lscr.io/linuxserver/thunderbird:1140.17.0"));
        assert!(script.contains("CUSTOM_USER=mail"));
        assert!(script.contains("PASSWORD=\"$pagepass\""));
        assert!(script.contains("chmod 0600 \"$cfg/page.pass\""));
        assert!(script.contains("\"$cfg/config:/config\""));
        assert!(script.contains("--shm-size=1g"));
        assert!(script.contains("-p 127.0.0.1:3000:3000"));
        assert!(!script.contains("0.0.0.0"));
        assert!(!script.contains("3001"));
        assert!(!script.contains("--privileged"));
        assert!(!script.contains("docker.sock"));
        assert!(!script.contains("9478"));
    }
}
