//! Keeps this computer answering when memory runs short.
//!
//! Compressed memory takes the overflow, the screen has a share it cannot go
//! past, and Keel and the file sharing are the last things the system stops.
//! A full screen then restarts on its own instead of the whole computer
//! locking up. Keel applies this at every start, so an update brings it to a
//! computer that is already installed.

pub fn guard() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    keep_alive_note();
    crate::storage::host("exec /bin/bash -s", &[], Some(script()))
}

const ALIVE: &str = "/var/lib/keel/alive";
pub const LAST_STOP: &str = "/var/lib/keel/last-stop";

/// Keel writes "<boot id> <time>" every minute. At the next start, a note
/// from another boot whose logs do not end in a shutdown means the computer
/// stopped without shutting down: it froze, the watchdog restarted it, or
/// the power went. That time goes to LAST_STOP for Repair.
fn keep_alive_note() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap_or_default()
        .trim()
        .to_string();
    if boot.is_empty() {
        return;
    }
    if let Ok(old) = std::fs::read_to_string(ALIVE) {
        if let Some(when) = stopped_unexpectedly(&old, &boot, &previous_boot_end()) {
            let _ = std::fs::write(LAST_STOP, format!("{when}\n"));
        }
    }
    std::thread::spawn(move || loop {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let tmp = format!("{ALIVE}.tmp");
        if std::fs::write(&tmp, format!("{boot} {now}\n")).is_ok() {
            let _ = std::fs::rename(&tmp, ALIVE);
        }
        std::thread::sleep(std::time::Duration::from_secs(60));
    });
}

/// The last lines the system wrote in the boot before this one. Empty when
/// those logs were only in memory.
fn previous_boot_end() -> String {
    std::process::Command::new("journalctl")
        .args(["-b", "-1", "-n", "40", "-o", "cat", "--no-pager", "-q"])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

/// The time Keel last answered, when the boot before this one ended without
/// shutting down. None when it shut down properly, when it is the same boot
/// (Keel itself restarted), or when there is nothing to tell by.
fn stopped_unexpectedly(note: &str, boot: &str, previous_end: &str) -> Option<u64> {
    let mut parts = note.split_whitespace();
    let (old_boot, when) = (parts.next()?, parts.next()?.parse::<u64>().ok()?);
    if old_boot == boot || previous_end.trim().is_empty() {
        return None;
    }
    let shut_down = ["Journal stopped", "Reached target System Power Off", "Reached target System Reboot", "Reached target System Halt", "Reached target Power-Off", "Reached target Reboot"]
        .iter()
        .any(|line| previous_end.contains(line));
    (!shut_down).then_some(when)
}

fn script() -> &'static str {
    include_str!("memory-guard.sh")
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_stop_without_shutting_down_is_noticed() {
        let froze = "Nov 17 kernel: usb 1-1.3: reset high-speed USB device\nkeel: picture copy 0.2 s\n";
        let clean = "Stopped target Basic System.\nsystemd-journald[201]: Journal stopped\n";
        assert_eq!(super::stopped_unexpectedly("aaa 1795000000\n", "bbb", froze), Some(1_795_000_000));
        assert_eq!(super::stopped_unexpectedly("aaa 1795000000\n", "bbb", clean), None);
        // Keel restarted, the computer did not.
        assert_eq!(super::stopped_unexpectedly("bbb 1795000000\n", "bbb", froze), None);
        // The logs of that boot were only in memory: nothing to tell by.
        assert_eq!(super::stopped_unexpectedly("aaa 1795000000\n", "bbb", ""), None);
        assert_eq!(super::stopped_unexpectedly("", "bbb", froze), None);
    }

    #[test]
    fn the_screen_gives_way_before_keel_and_the_files() {
        let script = super::script();
        assert!(script.contains("MemoryMax=50%"));
        assert!(script.contains("keel-kiosk.service.d/memory.conf"));
        assert!(script.contains("OOMScoreAdjust=-600"));
        assert!(script.contains("zramctl --find"));
        assert!(script.contains("WantedBy=swap.target"));
    }

    #[test]
    fn a_frozen_computer_restarts_and_apps_give_way_first() {
        let script = super::script();
        assert!(script.contains("RuntimeWatchdogSec=15s"));
        assert!(script.contains("machine.slice.d/90-keel.conf"));
        assert!(script.contains("MemoryMax=65%"));
        assert!(script.contains("keel-memory-watch.service"));
        assert!(script.contains("jam_for=6"));
        // Names and the front door are never chosen to stop.
        assert!(!script.contains("keel-door*)"));
        assert!(script.contains("systemd.show_status=auto"));
        assert!(script.contains("RuntimeMaxUse=16M"));
        assert!(script.contains("Storage=persistent"));
        assert!(script.contains("\"DiskCacheSize\": 33554432"));
        assert!(script.contains("SystemMaxUse=64M"));
    }
}
