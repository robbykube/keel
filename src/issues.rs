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
        items.extend(last_stop_issue(&std::fs::read_to_string(crate::memory::LAST_STOP).unwrap_or_default(), now_seconds()));
        items.extend(space_issues(&df_text()));
        items.extend(clock_issue(&clock_synced(), uptime_seconds()));
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
            id: "empty-run",
            label: "Empty the temporary space",
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
    if what.starts_with("full ") {
        return Some(Issue {
            id: "memory-stopped",
            severity: "red",
            title: "Memory stays full",
            why: "Memory stayed full even after Keel stopped an app. The computer restarted for this less than six hours ago, so Keel left it on this time. Close an app you do not need on Apps, or pick its light version in Settings.",
            evidence: line.to_string(),
            repair: None,
        });
    }
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

/// The computer stopped without shutting down in the last three days.
fn last_stop_issue(note: &str, now: i64) -> Option<Issue> {
    let when: i64 = note.trim().parse().ok()?;
    if now - when > 3 * 24 * 3600 {
        return None;
    }
    let said = std::process::Command::new("date")
        .args(["-d", &format!("@{when}"), "+%A %-d %B at %H:%M"])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| when.to_string());
    Some(Issue {
        id: "stopped-unexpectedly",
        severity: "yellow",
        title: "The computer stopped without shutting down",
        why: "If nobody switched it off, it froze or lost power. Logs shows the last lines from before it stopped, under Before the last stop. If this keeps happening, check the power supply first.",
        evidence: format!("last answered {said}"),
        repair: None,
    })
}

fn df_text() -> String {
    std::process::Command::new("df")
        .args(["-Pk"])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

/// The system disk, the Keel disks, and the temporary space, when they are
/// nearly full. Full temporary space stops every task (0.1.63).
fn space_issues(df: &str) -> Vec<Issue> {
    const MB: u64 = 1024;
    let mut items = Vec::new();
    for line in df.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 {
            continue;
        }
        let (Ok(size), Ok(avail)) = (cols[1].parse::<u64>(), cols[3].parse::<u64>()) else {
            continue;
        };
        let mount = cols[5];
        if size == 0 {
            continue;
        }
        let percent = avail * 100 / size;
        let found = match mount {
            "/" if avail < 300 * MB => Some(("system-full", "red", "The system disk is full", "Apps, updates and logs stop when the system disk is full. Remove apps you do not use on Apps, or move Media to a Keel disk.")),
            "/" if avail < 1536 * MB => Some(("system-low", "yellow", "The system disk is almost full", "Apps, updates and logs stop when the system disk fills. Remove apps you do not use on Apps, or move Media to a Keel disk.")),
            "/run" if avail < 8 * MB => Some(("run-full", "red", "The temporary space is full", "Keel cannot start tasks when the temporary space is full. Press Empty the temporary space below.")),
            "/run" if avail < 32 * MB => Some(("run-low", "yellow", "The temporary space is almost full", "Keel empties it by itself each minute. If this stays, press Empty the temporary space below. Logs shows what takes it.")),
            m if m.starts_with("/srv/keel/") && percent < 2 => Some(("disk-full", "red", "A Keel disk is full", "New files and copies cannot be saved on this disk. Delete files you do not need, or give the job to a larger disk on Disks.")),
            m if m.starts_with("/srv/keel/") && percent < 10 => Some(("disk-low", "yellow", "A Keel disk is almost full", "New files and copies stop when this disk fills. Delete files you do not need, or give the job to a larger disk on Disks.")),
            _ => None,
        };
        if let Some((id, severity, title, why)) = found {
            let mut evidence = format!("{mount}: {} MB free of {} MB ({percent}%)", avail / MB, size / MB);
            if mount == "/run" && cfg!(target_os = "linux") {
                let tenants = crate::storage::run_tenants();
                if !tenants.is_empty() {
                    evidence.push_str(&format!(". Largest: {tenants}"));
                }
            }
            items.push(Issue {
                id,
                severity,
                title,
                why,
                evidence,
                repair: None,
            });
        }
    }
    items
}

fn clock_synced() -> String {
    std::process::Command::new("timedatectl")
        .args(["show", "-p", "NTPSynchronized", "--value"])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default()
}

fn uptime_seconds() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|text| text.split('.').next()?.parse().ok())
        .unwrap_or(0)
}

/// A Pi has no clock battery. It sets the time from the internet after it
/// starts; until then the locks on the app names and update checks fail.
fn clock_issue(synced: &str, uptime: u64) -> Option<Issue> {
    (synced == "no" && uptime > 600).then(|| Issue {
        id: "clock-unset",
        severity: "yellow",
        title: "The clock is not set",
        why: "This computer sets its clock from the internet, and has not yet. Until it does, app names can lose their lock and updates cannot be checked. Check that the internet works in the house.",
        evidence: "NTPSynchronized=no".into(),
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
    fn space_is_warned_about_before_it_runs_out() {
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
/dev/mmcblk0p2 30000000 29500000 500000 99% /\n\
tmpfs 400000 380000 20000 95% /run\n\
/dev/sda1 1000000000 50000000 950000000 5% /srv/keel/files\n\
/dev/sdb1 1000000000 995000000 5000000 99% /srv/keel/copies\n\
/dev/sdc1 100 100 0 100% /media/usb\n";
        let found: Vec<(&str, &str)> = super::space_issues(df).iter().map(|i| (i.id, i.severity)).collect();
        assert_eq!(found, vec![("system-low", "yellow"), ("run-low", "yellow"), ("disk-full", "red")]);
        assert!(super::space_issues("").is_empty());
    }

    #[test]
    fn an_unset_clock_waits_ten_minutes() {
        assert!(super::clock_issue("no", 60).is_none());
        assert!(super::clock_issue("yes", 6000).is_none());
        assert!(super::clock_issue("", 6000).is_none());
        assert_eq!(super::clock_issue("no", 6000).unwrap().id, "clock-unset");
    }

    #[test]
    fn an_unexpected_stop_shows_for_three_days() {
        let now = 1_795_100_000;
        let issue = super::last_stop_issue("1795000000\n", now).unwrap();
        assert!(issue.evidence.starts_with("last answered "));
        assert!(super::last_stop_issue("1795000000", now + 4 * 24 * 3600).is_none());
        assert!(super::last_stop_issue("", now).is_none());
    }

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
        let left_on = super::stopped_issue("2026-11-18T18:30:00+01:00 full Memory stayed full; the computer restarted less than six hours ago, so it was left on.\n", at).unwrap();
        assert_eq!(left_on.severity, "red");
        assert!(left_on.why.contains("left it on"));
    }
}
