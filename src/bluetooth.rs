//! Keyboards and headphones. One look, then pair, connect, or forget.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

const SCRIPT: &str = include_str!("bluetooth.sh");
const JOB: &str = "/var/lib/keel/bluetooth-job";
const ANSWER: &str = "/var/lib/keel/bluetooth-answer";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Device {
    pub mac: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Report {
    pub message: String,
    pub state: String,
    pub code: String,
    /// "yes" asks for Yes or No. "pin" asks for a code typed here.
    /// "keyboard" means type the code on the keyboard being paired.
    pub ask: String,
    pub mac: String,
    pub devices: Vec<Device>,
}

static BUSY: Mutex<()> = Mutex::new(());
static LAST: Mutex<Option<Report>> = Mutex::new(None);

pub fn view() -> Report {
    let _hold = match BUSY.try_lock() {
        Ok(guard) => guard,
        Err(_) => return waiting(),
    };
    match finish(run("status", "")) {
        Ok(report) => report,
        Err(message) => {
            let mut report = idle();
            report.message = message;
            report
        }
    }
}

pub fn look() -> Result<Report, String> {
    let _hold = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    finish(run("scan", ""))
}

pub fn pair(mac: &str) -> Result<Report, String> {
    let mac = check_mac(mac)?;
    let _hold = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    finish(run("pair", &mac))
}

pub fn connect(mac: &str) -> Result<Report, String> {
    act("connect", mac)
}

pub fn disconnect(mac: &str) -> Result<Report, String> {
    act("disconnect", mac)
}

pub fn forget(mac: &str) -> Result<Report, String> {
    act("forget", mac)
}

/// Yes, no, or the code typed on this screen.
pub fn answer(text: &str) -> Result<(), String> {
    let text = text.trim();
    let ok = text == "yes"
        || text == "no"
        || (text.len() <= 16 && !text.is_empty() && text.chars().all(|c| c.is_ascii_digit()));
    if !ok {
        return Err("Type the code, or press Yes.".into());
    }
    fs::create_dir_all("/var/lib/keel").map_err(|e| e.to_string())?;
    fs::write(ANSWER, format!("{text}\n")).map_err(|e| e.to_string())
}

fn act(action: &str, mac: &str) -> Result<Report, String> {
    let mac = check_mac(mac)?;
    let _hold = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    finish(run(action, &mac))
}

fn finish(result: Result<String, String>) -> Result<Report, String> {
    let text = result?;
    let mut report = parse(&text);
    overlay_job(&mut report);
    if let Ok(mut last) = LAST.lock() {
        *last = Some(report.clone());
    }
    Ok(report)
}

fn waiting() -> Report {
    let mut report = LAST.lock().ok().and_then(|g| g.clone()).unwrap_or_else(idle);
    overlay_job(&mut report);
    if report.state == "idle" {
        report.state = "pairing".into();
        report.message = "Working.".into();
    }
    report
}

fn idle() -> Report {
    Report {
        message: "Press Look.".into(),
        state: "idle".into(),
        code: String::new(),
        ask: String::new(),
        mac: String::new(),
        devices: Vec::new(),
    }
}

fn run(action: &str, mac: &str) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("This runs on the Keel computer.".into());
    }
    let args: Vec<&str> = if mac.is_empty() { vec![action] } else { vec![action, mac] };
    crate::storage::host_all("exec /bin/bash -s", &args, Some(SCRIPT))
}

fn check_mac(mac: &str) -> Result<String, String> {
    let mac = mac.trim().to_ascii_uppercase();
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() == 6 && parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit())) {
        return Ok(mac);
    }
    Err("That is not a Bluetooth device.".into())
}

fn parse(text: &str) -> Report {
    let mut report = idle();
    report.message.clear();
    for line in text.lines() {
        let mut parts = line.splitn(5, '|');
        let kind = parts.next().unwrap_or("");
        match kind {
            "message" => report.message = parts.next().unwrap_or("").trim().to_string(),
            "device" => {
                let raw = parts.next().unwrap_or("").trim();
                let Ok(mac) = check_mac(raw) else { continue };
                let name = parts.next().unwrap_or("").trim().to_string();
                let paired = parts.next().unwrap_or("") == "yes";
                let connected = parts.next().unwrap_or("") == "yes";
                report.devices.push(Device {
                    mac,
                    name,
                    paired,
                    connected,
                });
            }
            _ => {}
        }
    }
    if report.message.is_empty() {
        report.message = "Press Look.".into();
    }
    report
}

fn overlay_job(report: &mut Report) {
    let Ok(text) = fs::read_to_string(JOB) else {
        return;
    };
    if !Path::new(JOB).exists() {
        return;
    }
    for line in text.lines() {
        let mut parts = line.splitn(2, '|');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("").trim();
        match key {
            "state" => report.state = value.to_string(),
            "code" => report.code = value.chars().filter(|c| c.is_ascii_digit()).take(6).collect(),
            "mac" if report.mac.is_empty() => report.mac = value.to_string(),
            _ => {}
        }
    }
    report.ask = match report.state.as_str() {
        "confirm" => "yes".into(),
        "pin" => "pin".into(),
        "keyboard" => "keyboard".into(),
        _ => String::new(),
    };
    if report.state == "looking" {
        report.message = "Looking for keyboards and headphones.".into();
    } else if report.state == "confirm" {
        report.message = "Press Yes if the device shows this code.".into();
    } else if report.state == "keyboard" {
        report.message = "Type this code on the keyboard, then press Enter.".into();
    } else if report.state == "pin" {
        report.message = "Type the code shown on the device.".into();
    } else if report.state == "pairing" {
        report.message = "Pairing.".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_keeps_paired_and_connected() {
        let text = "\
message|Press Pair on the keyboard or headphones. Put it in pairing mode first.
device|aa:bb:cc:dd:ee:ff|Living room|yes|no
device|11:22:33:44:55:66|Headphones|yes|yes
device|not-a-mac|Nope|yes|yes
";
        let report = parse(text);
        assert_eq!(report.devices.len(), 2);
        assert_eq!(report.devices[0].mac, "AA:BB:CC:DD:EE:FF");
        assert!(report.devices[0].paired && !report.devices[0].connected);
        assert!(report.devices[1].connected);
        assert!(report.message.contains("pairing mode"));
    }

    #[test]
    fn a_bad_address_is_refused() {
        assert!(check_mac("AA:BB:CC:DD:EE:FF").is_ok());
        assert!(check_mac("aa:bb:cc:dd:ee:ff").is_ok());
        assert!(check_mac("AA:BB:CC:DD:EE").is_err());
        assert!(check_mac("AA:BB:CC:DD:EE:FF;rm").is_err());
    }

    #[test]
    fn the_script_pairs_and_does_not_trust_a_typed_command() {
        assert!(SCRIPT.contains("agent KeyboardDisplay"));
        assert!(SCRIPT.contains("bluetoothctl --timeout 12 scan on"));
        assert!(SCRIPT.contains("Confirm passkey"));
        assert!(!SCRIPT.contains("eval "));
        assert!(!SCRIPT.contains("bash -c"));
    }
}
