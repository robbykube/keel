use crate::model::{Disk, InstallerPlan, InstallerStatus};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

const MIN_INSTALL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const INSTALLER_HELPER: &str = "/usr/lib/keel/install-keel";
const LIVE_SQUASHFS: &str = "/run/live/medium/live/filesystem.squashfs";
const STATUS_PATH: &str = "/run/keel/install.json";

static INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct InstallProgress {
    pub state: String,
    pub percent: u8,
    pub message: String,
    #[serde(default)]
    pub token: Option<String>,
}

impl InstallProgress {
    pub fn idle() -> Self {
        Self {
            state: "idle".into(),
            percent: 0,
            message: String::new(),
            token: None,
        }
    }

    pub fn for_client(mut self) -> Self {
        if self.state != "done" {
            self.token = None;
        }
        self
    }
}

pub fn status(disks: Vec<Disk>) -> InstallerStatus {
    let enabled = cfg!(target_os = "linux")
        && std::env::var("KEEL_INSTALLER_ENABLED").as_deref() == Ok("1")
        && Path::new(INSTALLER_HELPER).is_file()
        && Path::new(LIVE_SQUASHFS).is_file();
    InstallerStatus {
        mode: if enabled { "install" } else { "try" },
        can_install: enabled,
        network: if cfg!(target_os = "linux") {
            "available"
        } else {
            "development-host"
        },
        warning: if enabled {
            "Installation erases the selected whole disk. Check its name, model, and size before continuing."
        } else {
            "This USB is in safe Try mode. Boot the reviewed keel-amd64-install.iso image to enable installation."
        },
        disks,
    }
}

pub fn progress() -> InstallProgress {
    read_progress().for_client()
}

fn read_progress() -> InstallProgress {
    if !cfg!(target_os = "linux") {
        return InstallProgress::idle();
    }
    match std::fs::read(STATUS_PATH) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| fallback_progress()),
        Err(_) => fallback_progress(),
    }
}

fn fallback_progress() -> InstallProgress {
    if INSTALLING.load(Ordering::Relaxed) {
        InstallProgress {
            state: "running".into(),
            percent: 1,
            message: "Starting the install.".into(),
            token: None,
        }
    } else {
        InstallProgress::idle()
    }
}

fn write_progress(progress: &InstallProgress) {
    if let Ok(body) = serde_json::to_vec(progress) {
        let tmp = format!("{STATUS_PATH}.tmp");
        if std::fs::write(&tmp, body).is_ok() {
            let _ = std::fs::rename(&tmp, STATUS_PATH);
        }
    }
}

pub fn validate_plan(plan: &InstallerPlan, disks: &[Disk]) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("Installation is only available when Keel is booted on Linux media.".into());
    }
    if std::env::var("KEEL_INSTALLER_ENABLED").as_deref() != Ok("1") {
        return Err("The installer is disabled in this build. Use Try mode while the installer is being tested.".into());
    }
    if plan.confirmation != "INSTALL KEEL" {
        return Err("Type INSTALL KEEL exactly after checking the disk name and size.".into());
    }
    let disk = disks
        .iter()
        .find(|disk| disk.path == plan.target)
        .ok_or_else(|| "Choose one of the whole disks detected by Keel.".to_string())?;
    if disk.kind != "disk" || disk.path.is_empty() {
        return Err("That is not a usable whole disk.".into());
    }
    if disk.size < MIN_INSTALL_BYTES {
        return Err("Keel needs at least 16 GB on the system disk.".into());
    }
    if !disk.mount.trim().is_empty() {
        return Err("That disk is mounted. Choose an unmounted system disk.".into());
    }
    Ok(())
}

pub fn start(plan: &InstallerPlan, disks: &[Disk]) -> Result<String, String> {
    validate_plan(plan, disks)?;
    if !Path::new(INSTALLER_HELPER).is_file() || !Path::new(LIVE_SQUASHFS).is_file() {
        return Err(
            "The reviewed installer image is not available. Keel remains in Try mode.".into(),
        );
    }
    let current = read_progress();
    if current.state == "running" || INSTALLING.load(Ordering::SeqCst) {
        return Err("An install is already running.".into());
    }
    INSTALLING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "An install is already running.".to_string())?;
    write_progress(&InstallProgress {
        state: "running".into(),
        percent: 1,
        message: "Starting the install.".into(),
        token: None,
    });
    let target = plan.target.clone();
    let confirm = plan.confirmation.clone();
    thread::spawn(move || {
        let result = Command::new(INSTALLER_HELPER)
            .arg(&target)
            .env("KEEL_INSTALL_CONFIRM", &confirm)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let after = read_progress();
        if after.state == "running" || after.state == "idle" {
            let message = match result {
                Ok(status) if status.success() => InstallProgress {
                    state: "done".into(),
                    percent: 100,
                    message: "Keel is on this disk. Remove the USB and restart.".into(),
                    token: after.token,
                },
                Ok(_) => InstallProgress {
                    state: "error".into(),
                    percent: after.percent,
                    message: "Install stopped. The disk may be incomplete. Run the install again on the same disk.".into(),
                    token: None,
                },
                Err(_) => InstallProgress {
                    state: "error".into(),
                    percent: 0,
                    message: "The installer could not start.".into(),
                    token: None,
                },
            };
            write_progress(&message);
        }
        INSTALLING.store(false, Ordering::SeqCst);
    });
    Ok("Install started.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(path: &str, size: u64, mount: &str) -> Disk {
        Disk {
            name: path.trim_start_matches("/dev/").into(),
            path: path.into(),
            size,
            kind: "disk".into(),
            model: "test".into(),
            transport: "virtio".into(),
            rotational: false,
            mount: mount.into(),
            fstype: "".into(),
        }
    }

    fn plan(target: &str) -> InstallerPlan {
        InstallerPlan {
            target: target.into(),
            confirmation: "INSTALL KEEL".into(),
        }
    }

    #[test]
    fn only_detected_whole_disks_can_be_selected() {
        let disks = vec![disk("/dev/vda", MIN_INSTALL_BYTES, "")];
        assert!(validate_plan(&plan("/dev/vda1"), &disks).is_err());
        assert!(validate_plan(&plan("/dev/vda"), &disks).is_err());
    }

    #[test]
    fn mounted_and_small_disks_are_rejected() {
        assert!(validate_plan(
            &plan("/dev/vda"),
            &[disk("/dev/vda", MIN_INSTALL_BYTES, "/")]
        )
        .is_err());
        assert!(validate_plan(
            &plan("/dev/vda"),
            &[disk("/dev/vda", MIN_INSTALL_BYTES - 1, "")]
        )
        .is_err());
    }

    #[test]
    fn progress_json_round_trip() {
        let raw = r#"{"state":"running","percent":42,"message":"Copying Keel onto the disk","token":null}"#;
        let progress: InstallProgress = serde_json::from_str(raw).unwrap();
        assert_eq!(progress.percent, 42);
        assert_eq!(progress.state, "running");
        assert!(progress.token.is_none());
    }

    #[test]
    fn token_stays_hidden_until_done() {
        let running = InstallProgress {
            state: "running".into(),
            percent: 10,
            message: "Copying Keel onto the disk".into(),
            token: Some("keel-secret".into()),
        }
        .for_client();
        assert!(running.token.is_none());

        let done = InstallProgress {
            state: "done".into(),
            percent: 100,
            message: "Keel is on this disk. Remove the USB and restart.".into(),
            token: Some("keel-secret".into()),
        }
        .for_client();
        assert_eq!(done.token.as_deref(), Some("keel-secret"));
    }
}
