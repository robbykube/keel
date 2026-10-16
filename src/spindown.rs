//! One try: is a disk in the USB box already asleep, and if not, will it sleep
//! when asked?
//!
//! The box may already spin disks down. This does not set a timer. It asks one
//! spinning disk, once, and says what happened. A check that finds the disk
//! already asleep does not wake it.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::model::Disk;

const SLEEP_SCRIPT: &str = r#"set -uo pipefail
dev="__DEV__"
say() { printf '%s\n' "$1"; exit 0; }
fail() { echo "$1" >&2; exit 1; }
case "$dev" in
  /dev/*) ;;
  *) fail "That disk is not one Keel can ask." ;;
esac
[ -b "$dev" ] || fail "That disk is not connected."
ensure() {
  command -v "$1" >/dev/null 2>&1 && return 0
  apt-get update -o Acquire::ForceIPv4=true >/dev/null 2>&1 || true
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true "$2" >/dev/null 2>&1
}
ensure smartctl smartmontools || fail "Keel could not install the disk check. The disks stay spinning."
ensure hdparm hdparm || fail "Keel could not install the sleep tool. The disks stay spinning."
smart=$(command -v smartctl)
hd=$(command -v hdparm)
set +e
out=$("$smart" -n standby --info "$dev" 2>&1)
asleep=0
printf '%s\n' "$out" | grep -qi 'standby mode\|sleep mode' || asleep=1
set -e
[ "$asleep" -eq 0 ] && say ALREADY
"$hd" -y "$dev" >/dev/null 2>&1 || say IGNORED
sleep 3
set +e
out=$("$smart" -n standby --info "$dev" 2>&1)
asleep=0
printf '%s\n' "$out" | grep -qi 'standby mode\|sleep mode' || asleep=1
set -e
[ "$asleep" -eq 0 ] && say SLEPT
say IGNORED
"#;

pub fn try_once(disks: &[Disk]) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Ok("This try runs on the Keel computer. This copy has no USB box to ask.".into());
    }
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("Keel is already asking a disk.".into());
    }
    let result = try_held(disks);
    BUSY.store(false, Ordering::SeqCst);
    result
}

fn try_held(disks: &[Disk]) -> Result<String, String> {
    let Some(disk) = choose(disks) else {
        return Err("There is no spinning disk in the USB box to ask.".into());
    };
    if !safe_device(&disk.path) {
        return Err("That disk is not one Keel can ask.".into());
    }
    let script = SLEEP_SCRIPT.replace("__DEV__", &disk.path);
    let word = crate::storage::host_text("exec /bin/bash -s", &[], Some(&script))?;
    Ok(sentence(&disk_label(disk), word.trim()))
}

pub fn choose(disks: &[Disk]) -> Option<&Disk> {
    let mut found: Vec<&Disk> = disks
        .iter()
        .filter(|disk| {
            disk.kind == "disk" && disk.rotational && disk.transport == "usb" && !disk.system && safe_device(&disk.path)
        })
        .collect();
    found.sort_by_key(|disk| rank(disk));
    found.into_iter().next()
}

fn rank(disk: &Disk) -> u8 {
    match disk.label.strip_prefix("keel-") {
        Some("copy" | "extra" | "software") => 0,
        Some("files") => 2,
        Some(_) => 1,
        None => 1,
    }
}

fn disk_label(disk: &Disk) -> String {
    if disk.model.is_empty() {
        disk.name.clone()
    } else {
        format!("{} ({})", disk.model, disk.name)
    }
}

pub fn sentence(label: &str, word: &str) -> String {
    match word {
        "ALREADY" => format!("{label} is already asleep. The USB box is doing that, so Keel will leave it alone."),
        "SLEPT" => format!("Keel asked {label} to sleep, and it did. Opening a file on it spins it back up."),
        _ => format!("The USB box ignored the sleep request for {label}. Keel will leave the disks spinning."),
    }
}

fn safe_device(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/dev/") else {
        return false;
    };
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(name: &str, label: &str, transport: &str, rotational: bool, system: bool) -> Disk {
        Disk {
            name: name.into(),
            path: format!("/dev/{name}"),
            size: 1,
            kind: "disk".into(),
            model: "Drive".into(),
            transport: transport.into(),
            rotational,
            mount: String::new(),
            fstype: String::new(),
            label: label.into(),
            system,
            empty: false,
            free: None,
            fs_size: None,
            health: String::new(),
        }
    }

    #[test]
    fn the_try_picks_a_copy_disk_before_the_files_disk() {
        let files = disk("sdb", "keel-files", "usb", true, false);
        let copy = disk("sdc", "keel-copy", "usb", true, false);
        let nvme = disk("nvme0n1", "", "nvme", false, true);
        let disks = [files, nvme, copy];
        let chosen = choose(&disks).unwrap();
        assert_eq!(chosen.name, "sdc");
    }

    #[test]
    fn the_system_disk_and_a_bad_path_are_left_alone() {
        let root = disk("sda", "KEEL-ROOT", "sata", false, true);
        let roots = [root];
        assert!(choose(&roots).is_none());
        let mut usb = disk("sdb", "keel-files", "usb", true, false);
        usb.path = "/dev/sdb;reboot".into();
        let bad = [usb];
        assert!(choose(&bad).is_none());
    }

    #[test]
    fn the_word_from_the_disk_becomes_one_sentence() {
        let already = sentence("Seagate (sdc)", "ALREADY");
        assert!(already.contains("already asleep"));
        assert!(already.contains("leave it alone"));
        let slept = sentence("Seagate (sdc)", "SLEPT");
        assert!(slept.contains("spins it back up"));
        let ignored = sentence("Seagate (sdc)", "IGNORED");
        assert!(ignored.contains("ignored"));
        assert!(ignored.contains("leave the disks spinning"));
    }
}
