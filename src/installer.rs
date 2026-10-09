use crate::model::{Disk, InstallerPlan, InstallerStatus};
use std::path::Path;
use std::process::Command;

const MIN_INSTALL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const INSTALLER_HELPER: &str = "/usr/lib/keel/install-keel";
const LIVE_SQUASHFS: &str = "/run/live/medium/live/filesystem.squashfs";

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
            "Try mode does not change internal disks. Installation is enabled only in the reviewed Linux image."
        },
        disks,
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

pub fn install(plan: &InstallerPlan, disks: &[Disk]) -> Result<String, String> {
    validate_plan(plan, disks)?;
    if !Path::new(INSTALLER_HELPER).is_file() || !Path::new(LIVE_SQUASHFS).is_file() {
        return Err(
            "The reviewed installer image is not available. Keel remains in Try mode.".into(),
        );
    }
    let result = Command::new(INSTALLER_HELPER)
        .arg(&plan.target)
        .env("KEEL_INSTALL_CONFIRM", &plan.confirmation)
        .status()
        .map_err(|e| format!("could not start the installer: {e}"))?;
    if !result.success() {
        return Err(format!(
            "installer stopped without completing (status {result})"
        ));
    }
    Ok(format!(
        "Keel was installed to {}. Remove the USB and reboot.",
        plan.target
    ))
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
}
