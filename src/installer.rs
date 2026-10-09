use crate::model::{Disk, InstallerPlan, InstallerStatus};

pub fn status(disks: Vec<Disk>) -> InstallerStatus {
    InstallerStatus {
        mode: "try",
        can_install: cfg!(target_os = "linux") && std::env::var("KEEL_INSTALLER_ENABLED").as_deref() == Ok("1"),
        network: if cfg!(target_os = "linux") { "available" } else { "development-host" },
        warning: "Try mode does not change internal disks. Installation will only be enabled after a separate, reviewed installer is included in the image.",
        disks,
    }
}

pub fn validate_plan(plan: &InstallerPlan) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("Installation is only available when Keel is booted on Linux media.".into());
    }
    if std::env::var("KEEL_INSTALLER_ENABLED").as_deref() != Ok("1") {
        return Err("The installer is disabled in this build. Use Try mode while the installer is being tested.".into());
    }
    if !plan.target.starts_with("/dev/") {
        return Err(
            "Choose an internal Linux disk, for example /dev/sda. Do not enter a partition.".into(),
        );
    }
    if plan.confirmation != "INSTALL KEEL" {
        return Err("Type INSTALL KEEL exactly after checking the disk name and size.".into());
    }
    Ok(format!(
        "Installation plan accepted for {}. No disk was changed.",
        plan.target
    ))
}
