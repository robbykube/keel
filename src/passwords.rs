//! Passwords is Vaultwarden, pinned, at passwords.<the name you own>.
//!
//! The front door holds the lock. Vaultwarden listens on this computer only.
//! The first account is created in the page. Close new accounts after that.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde::Serialize;

const PROGRESS_PATH: &str = "/run/keel/passwords.json";
const INSTALLED: &str = "/var/lib/vaultwarden/installed";

static OPENING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
pub struct Status {
    pub ready: bool,
    pub address: String,
    pub message: String,
    pub state: String,
    pub percent: u8,
    pub signups_open: bool,
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
    #[serde(default)]
    signups_open: bool,
}

pub fn view() -> Status {
    if !cfg!(target_os = "linux") {
        return Status {
            ready: false,
            address: String::new(),
            message: "Passwords install on the Keel computer.".into(),
            state: "unavailable".into(),
            percent: 0,
            signups_open: false,
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
                signups_open: saved.signups_open,
            };
        }
    }
    idle()
}

pub fn start(close: bool) -> Result<Status, String> {
    if !cfg!(target_os = "linux") {
        return Err("Passwords install on the Keel computer.".into());
    }
    if close && !Path::new(INSTALLED).is_file() {
        return Err("Open Passwords first.".into());
    }
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    let address = password_address(&owned)?;
    if OPENING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(view());
    }
    let opening_line = if close {
        "Closing new accounts."
    } else {
        "Opening Names."
    };
    write_progress("running", 2, opening_line, "", false, false);
    let host = address.trim_start_matches("https://").to_string();
    let script = setup_script(&host, close);
    std::thread::spawn(move || {
        if let Err(message) = crate::names::ensure() {
            write_progress("error", 0, &message, "", false, false);
            OPENING.store(false, Ordering::SeqCst);
            return;
        }
        write_progress("running", 36, "Opening Passwords.", "", false, false);
        let result = crate::storage::host_text("exec /bin/bash -s", &[], Some(&script));
        match result {
            Ok(word) if word == "open" || word == "closed" => {
                let signups_open = word == "open";
                write_progress("running", 92, "Writing the name.", "", false, signups_open);
                match crate::names::publish("passwords", "keel-passwords", 80) {
                    Ok(waiting) => write_progress(
                        "done",
                        100,
                        &ready_message(&address, signups_open, waiting),
                        &address,
                        true,
                        signups_open,
                    ),
                    Err(message) => write_progress("error", 0, &message, "", false, false),
                }
            }
            Ok(_) => write_progress(
                "error",
                0,
                "Keel could not tell whether new accounts are open.",
                "",
                false,
                false,
            ),
            Err(message) => write_progress("error", 0, &message, "", false, false),
        }
        OPENING.store(false, Ordering::SeqCst);
    });
    Ok(view())
}

pub fn password_address(owned: &str) -> Result<String, String> {
    crate::owned::app_address(owned, "passwords")
}

fn ready_message(address: &str, signups_open: bool, waiting: bool) -> String {
    let lock = crate::names::lock_note(waiting);
    if signups_open {
        format!(
            "Passwords are at {address}. Open that and create the account for this house. In the Bitwarden app, set the server to that same address. Then press Close new accounts.{lock} You do not type addresses."
        )
    } else {
        format!(
            "Passwords are at {address}. New accounts are closed. In the Bitwarden app, set the server to that same address.{lock} You do not type addresses."
        )
    }
}

fn idle() -> Status {
    let installed = Path::new(INSTALLED).is_file();
    Status {
        ready: false,
        address: String::new(),
        message: if installed {
            "Passwords are on this computer. Press Open to start them.".into()
        } else {
            "Press Open. Keel makes the passwords name and asks for the lock. The first time downloads the program.".into()
        },
        state: "idle".into(),
        percent: 0,
        signups_open: false,
    }
}

fn read_progress() -> Option<Saved> {
    let bytes = fs::read(PROGRESS_PATH).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_progress(
    state: &str,
    percent: u8,
    message: &str,
    address: &str,
    ready: bool,
    signups_open: bool,
) {
    let saved = Saved {
        state: state.into(),
        percent,
        message: message.into(),
        address: address.into(),
        ready,
        signups_open,
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

pub fn setup_script(host: &str, close: bool) -> String {
    let mut script = include_str!("passwords-setup.sh").to_string();
    script = script.replace("__KEEL_HOST__", host);
    script = script.replace("__KEEL_CLOSE__", if close { "yes" } else { "no" });
    script
}

#[cfg(test)]
mod tests {
    use super::{password_address, setup_script};

    #[test]
    fn the_password_address_is_the_bought_name() {
        assert_eq!(
            password_address("klaingar.com").as_deref(),
            Ok("https://passwords.klaingar.com")
        );
        assert!(password_address("").is_err());
        assert!(password_address("keel.example.ts.net").is_err());
        assert!(password_address("klaingar.com/admin").is_err());
    }

    #[test]
    fn the_setup_pins_the_program_and_keeps_it_off_the_house_address() {
        let script = setup_script("passwords.klaingar.com", false);
        assert!(script.contains("docker.io/vaultwarden/server:1.37.4"));
        assert!(script.contains("host='passwords.klaingar.com'"));
        assert!(script.contains("close='no'"));
        assert!(script.contains("DOMAIN=\"https://$host\""));
        assert!(script.contains("SIGNUPS_ALLOWED=\"$signups\""));
        assert!(script.contains("SIGNUPS_VERIFY=false"));
        assert!(script.contains("-p 127.0.0.1:8081:80"));
        assert!(!script.contains("nginx"));
        assert!(!script.contains("8444"));
        assert!(!script.contains("ROCKET_TLS"));
        assert!(!script.contains("tailscale"));
        assert!(!script.contains("0.0.0.0"));
        assert!(!script.contains("9478"));
        assert!(!script.contains(":443:"));
    }

    #[test]
    fn closing_writes_the_flag_before_the_program_starts() {
        let script = setup_script("passwords.klaingar.com", true);
        assert!(script.contains("close='yes'"));
        let close_at = script.find("touch \"$cfg/signups.closed\"").unwrap();
        let run_at = script.find("podman run -d --name keel-passwords ").unwrap();
        assert!(close_at < run_at);
    }
}
