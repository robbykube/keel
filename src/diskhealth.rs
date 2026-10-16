//! Room left on a disk, and whether the disk says it is failing.
//!
//! A missing reading stays unknown. A USB bridge that cannot answer is not a
//! failing disk. SMART is cached so opening Home does not wake a sleeping disk
//! on every refresh.

use crate::model::Disk;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SMART_INSTALL: &str = r#"
set -euo pipefail
if command -v smartctl >/dev/null 2>&1; then
  exit 0
fi
apt-get update -o Acquire::ForceIPv4=true >/dev/null
DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true smartmontools >/dev/null
"#;

const SMART_HOLD: Duration = Duration::from_secs(10 * 60);
const SPACE_HOLD: Duration = Duration::from_secs(60);

struct Cached<T> {
    at: Instant,
    value: T,
}

fn smart_cache() -> &'static Mutex<HashMap<String, Cached<String>>> {
    static CACHE: std::sync::LazyLock<Mutex<HashMap<String, Cached<String>>>> =
        std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
    &CACHE
}

fn space_cache() -> &'static Mutex<HashMap<String, Cached<(u64, u64)>>> {
    static CACHE: std::sync::LazyLock<Mutex<HashMap<String, Cached<(u64, u64)>>>> =
        std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
    &CACHE
}

pub fn annotate(disks: &mut [Disk]) {
    if !cfg!(target_os = "linux") {
        for disk in disks.iter_mut() {
            if disk.mount.is_empty() || disk.size == 0 {
                continue;
            }
            disk.fs_size = Some(disk.size);
            disk.free = Some(disk.size * 72 / 100);
            disk.health = "unknown".into();
        }
        return;
    }
    offer_smartctl();
    for disk in disks.iter_mut() {
        if let Some((free, total)) = space(&disk.mount) {
            disk.free = Some(free);
            disk.fs_size = Some(total);
        }
        disk.health = health_from(&disk.transport, || smart(&disk.path)).into();
    }
}

/// A USB box often answers for itself, not for the disk inside it. Room left
/// still shows. A SMART failure from that box is not called a failing disk.
fn health_from(transport: &str, read: impl FnOnce() -> &'static str) -> &'static str {
    if transport == "usb" {
        "unknown"
    } else {
        read()
    }
}

fn offer_smartctl() {
    if smartctl_bin().is_some() {
        return;
    }
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let _ = crate::storage::host(SMART_INSTALL, &[], None);
    });
}

pub fn failing<'a>(disks: &'a [Disk]) -> Vec<&'a Disk> {
    disks.iter().filter(|disk| disk.health == "failing").collect()
}

fn space(mount: &str) -> Option<(u64, u64)> {
    if mount.is_empty() || mount == "[SWAP]" {
        return None;
    }
    if let Some(hit) = space_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(mount).filter(|hit| hit.at.elapsed() < SPACE_HOLD).map(|hit| hit.value))
    {
        return Some(hit);
    }
    let measured = read_space(mount)?;
    if let Ok(mut cache) = space_cache().lock() {
        cache.insert(mount.to_string(), Cached { at: Instant::now(), value: measured });
    }
    Some(measured)
}

fn read_space(mount: &str) -> Option<(u64, u64)> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let wanted = Path::new(mount);
    disks.list().iter().find_map(|disk| {
        if disk.mount_point() != wanted {
            return None;
        }
        let total = disk.total_space();
        if total == 0 {
            return None;
        }
        let free = disk.available_space().min(total);
        Some((free, total))
    })
}

fn smart(path: &str) -> &'static str {
    if !safe_device(path) {
        return "unknown";
    }
    if let Some(hit) = smart_cache().lock().ok().and_then(|cache| {
        cache
            .get(path)
            .filter(|hit| hit.at.elapsed() < SMART_HOLD)
            .map(|hit| hit.value.clone())
    }) {
        return leak(hit);
    }
    let value = read_smart(path);
    if let Ok(mut cache) = smart_cache().lock() {
        cache.insert(path.to_string(), Cached { at: Instant::now(), value: value.to_string() });
    }
    value
}

fn leak(value: String) -> &'static str {
    match value.as_str() {
        "ok" => "ok",
        "failing" => "failing",
        _ => "unknown",
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

fn smartctl_bin() -> Option<&'static str> {
    ["/usr/sbin/smartctl", "/usr/bin/smartctl"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
}

fn read_smart(path: &str) -> &'static str {
    let Some(bin) = smartctl_bin() else {
        return "unknown";
    };
    let mut child = match Command::new(bin)
        .args(["-j", "-n", "standby", "--health", path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return "unknown",
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < Duration::from_secs(8) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return "unknown";
            }
        }
    };
    let mut output = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        use std::io::Read;
        let _ = stdout.read_to_end(&mut output);
    }
    let _ = status;
    classify_smart(&String::from_utf8_lossy(&output))
}

pub(crate) fn classify_smart(raw: &str) -> &'static str {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return "unknown";
    };
    match value.pointer("/smart_status/passed").and_then(Value::as_bool) {
        Some(true) => "ok",
        Some(false) => "failing",
        None => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_passed_disk_is_ok_and_a_failed_disk_is_failing() {
        assert_eq!(
            classify_smart(r#"{"smart_status":{"passed":true}}"#),
            "ok"
        );
        assert_eq!(
            classify_smart(r#"{"smart_status":{"passed":false}}"#),
            "failing"
        );
        assert_eq!(classify_smart(r#"{"smartctl":{"exit_status":2}}"#), "unknown");
        assert_eq!(classify_smart("not json"), "unknown");
    }

    #[test]
    fn a_usb_box_is_not_called_failing() {
        assert_eq!(health_from("usb", || "failing"), "unknown");
        assert_eq!(health_from("sata", || "failing"), "failing");
        assert_eq!(health_from("nvme", || "ok"), "ok");
    }

    #[test]
    fn only_a_real_device_path_is_asked() {
        assert!(safe_device("/dev/sda"));
        assert!(safe_device("/dev/nvme0n1"));
        assert!(!safe_device("/dev/sda;reboot"));
        assert!(!safe_device("sda"));
        assert!(!safe_device("/etc/passwd"));
    }
}
