use crate::inventory::Inventory;
use crate::model::{Issue, IssuesReport, RepairAction, RepairResult};

pub fn report(
    idle: bool,
    inventory: Inventory,
    pool_count: usize,
    share_count: usize,
) -> IssuesReport {
    let mut items = Vec::new();

    if matches!(inventory, Inventory::Mock) {
        items.push(Issue {
            id: "mock-inventory",
            severity: "yellow",
            title: "This GUI is the control-plane host, not the NAS yet",
            why: "Windows has no lsblk. Disks are mock. Repair is: install keel on the Debian box, not a button here.",
            evidence: "inventory=mock".into(),
            repair: None,
        });
    }

    if pool_count == 0 {
        items.push(Issue {
            id: "no-pool",
            severity: "yellow",
            title: "No disk for files yet",
            why: "Files are kept on the system disk for now. Open Disks and give a disk the job of keeping files.",
            evidence: "pools=0".into(),
            repair: None,
        });
    }

    if share_count == 0 && !idle {
        items.push(Issue {
            id: "no-share",
            severity: "yellow",
            title: "No shared folder",
            why: "Keel is on, but there is no folder for other computers to open. Open Files and add one.",
            evidence: "shares=0".into(),
            repair: None,
        });
    }

    if matches!(inventory, Inventory::Linux) {
        items.extend(power_issue(power_now(), &kernel_messages()));
        items.extend(stopped_issue(&std::fs::read_to_string("/var/lib/keel/memory-watch.log").unwrap_or_default(), now_seconds()));
    }

    let playbook = vec![
        RepairAction {
            id: "rescan-disks",
            label: "Rescan disks",
            safe: true,
            available: true,
        },
        RepairAction {
            id: "restart-keel",
            label: "Restart Keel",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
        RepairAction {
            id: "restart-samba",
            label: "Restart file sharing",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
        RepairAction {
            id: "restart-tailscale",
            label: "Restart Away",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
        RepairAction {
            id: "open-ports",
            label: "Open the page and files to the house",
            safe: true,
            available: false,
        },
        RepairAction {
            id: "house-only",
            label: "Only the house and Away",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
    ];

    IssuesReport {
        red: items.iter().any(|i| i.severity == "red"),
        items,
        playbook,
    }
}

pub fn apply(id: &str) -> RepairResult {
    match id {
        "rescan-disks" => RepairResult {
            ok: true,
            id: id.into(),
            message: "Keel looked for disks again.".into(),
        },
        "restart-samba" | "restart-tailscale" | "open-ports" => RepairResult {
            ok: false,
            id: id.into(),
            message: "This works on the Keel computer itself.".into(),
        },
        other => RepairResult {
            ok: false,
            id: other.into(),
            message: "Unknown repair.".into(),
        },
    }
}

/// Whether the Pi says its power is too low right now: the rpi_volt sensor's
/// alarm. Other computers have no such sensor.
fn power_now() -> bool {
    let Ok(dirs) = std::fs::read_dir("/sys/class/hwmon") else {
        return false;
    };
    dirs.flatten().any(|dir| {
        let path = dir.path();
        std::fs::read_to_string(path.join("name")).is_ok_and(|name| name.trim() == "rpi_volt")
            && std::fs::read_to_string(path.join("in0_lcrit_alarm")).is_ok_and(|alarm| alarm.trim() == "1")
    })
}

/// The kernel's notes since this start, newest last.
fn kernel_messages() -> String {
    std::process::Command::new("journalctl")
        .args(["-k", "-b", "-o", "cat", "--no-pager", "-n", "3000"])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

/// A weak power supply freezes a Pi and stops disks. Low now is red; low
/// earlier since this start is worth a look.
fn power_issue(low_now: bool, kernel: &str) -> Option<Issue> {
    let times = kernel
        .lines()
        .filter(|line| {
            let line = line.to_ascii_lowercase();
            line.contains("undervoltage detected") || line.contains("under-voltage detected")
        })
        .count();
    if low_now {
        return Some(Issue {
            id: "power-low",
            severity: "red",
            title: "The power supply is too weak",
            why: "The Pi is getting too little power right now. That makes it freeze and drop disks. Use the official 5.1 V 3 A Pi supply, and give USB disks their own power.",
            evidence: format!("low now; {times} times since this start"),
            repair: None,
        });
    }
    (times > 0).then(|| Issue {
        id: "power-dipped",
        severity: "yellow",
        title: "The power dipped",
        why: "The Pi got too little power for a moment since it started. If it freezes, the power supply is the first thing to change: use the official 5.1 V 3 A Pi supply, and give USB disks their own power.",
        evidence: format!("{times} times since this start"),
        repair: None,
    })
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// What the memory watch did in the last day: an app it stopped, or a
/// restart. Lines look like "2026-11-18T18:13:54+01:00 stopped Photos".
fn stopped_issue(log: &str, now: i64) -> Option<Issue> {
    let line = log.lines().rev().find(|line| {
        line.split_whitespace()
            .next()
            .and_then(seconds_of)
            .is_some_and(|when| now - when < 24 * 3600)
    })?;
    let (_, what) = line.split_once(' ')?;
    let why = match what.strip_prefix("stopped ") {
        Some("Photos") => "Memory was full for three minutes, so Keel stopped Photos to keep the computer answering. Open it on Apps when you need it.",
        Some("Video") => "Memory was full for three minutes, so Keel stopped Video to keep the computer answering. Open it on Apps when you need it.",
        Some("Jellyfin") => "Memory was full for three minutes, so Keel stopped Jellyfin to keep the computer answering. Open it on Apps when you need it.",
        Some("Passwords") => "Memory was full for three minutes, so Keel stopped Passwords to keep the computer answering. Open it on Apps when you need it.",
        Some("Papers") => "Memory was full for three minutes, so Keel stopped Papers to keep the computer answering. Open it on Apps when you need it.",
        Some("Mail") => "Memory was full for three minutes, so Keel stopped Mail to keep the computer answering. Open it on Apps when you need it.",
        Some(_) => "Memory was full for three minutes, so Keel stopped an app to keep the computer answering. Open it on Apps when you need it.",
        None => "Memory stayed full even after Keel stopped an app, so Keel restarted the computer.",
    };
    Some(Issue {
        id: "memory-stopped",
        severity: "yellow",
        title: "Memory ran out",
        why,
        evidence: line.to_string(),
        repair: None,
    })
}

/// Seconds since 1970 for "2026-11-18T18:13:54+01:00" (what date -Is writes).
fn seconds_of(stamp: &str) -> Option<i64> {
    let num = |range: std::ops::Range<usize>| stamp.get(range)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let sign = match stamp.get(19..20)? { "+" => 1, "-" => -1, "Z" => 0, _ => return None };
    let offset = if sign == 0 { 0 } else { sign * (num(20..22)? * 3600 + num(23..25)? * 60) };
    // Days from the civil date (Howard Hinnant's method).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + sec - offset)
}

#[cfg(test)]
mod power_tests {
    #[test]
    fn weak_power_is_said_plainly() {
        assert!(super::power_issue(false, "usb 1-1: new device\n").is_none());
        let dipped = super::power_issue(false, "hwmon hwmon1: Undervoltage detected!\nhwmon hwmon1: Voltage normalised\n").unwrap();
        assert_eq!(dipped.severity, "yellow");
        assert_eq!(dipped.evidence, "1 times since this start");
        assert_eq!(super::power_issue(true, "").unwrap().severity, "red");
    }

    #[test]
    fn a_stopped_app_shows_for_a_day() {
        let log = "2026-11-17T09:00:00+01:00 stopped Video\n2026-11-18T18:13:54+01:00 stopped Photos\n";
        assert_eq!(super::seconds_of("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(super::seconds_of("2026-11-18T19:00:00Z"), Some(1_795_028_400));
        let at = super::seconds_of("2026-11-18T20:00:00+01:00").unwrap();
        let issue = super::stopped_issue(log, at).unwrap();
        assert!(issue.why.contains("stopped Photos"));
        assert!(super::stopped_issue(log, at + 2 * 24 * 3600).is_none());
        let restart = super::stopped_issue("2026-11-18T18:20:00+01:00 restart Memory stayed full after an app was stopped.\n", at).unwrap();
        assert!(restart.why.contains("restarted the computer"));
    }
}
