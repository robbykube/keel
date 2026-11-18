//! Names is Technitium. The front door is Caddy on this computer.
//!
//! Both listen on the house address. The locked page keeps port 443 on the
//! Away address, so a proxy never sits in front of Keel. No proxy host is
//! created for this page.

use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use serde::Serialize;

static NAMES_OPENING: AtomicBool = AtomicBool::new(false);
static DOOR_OPENING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
pub struct Status {
    pub ready: bool,
    pub address: String,
    pub message: String,
    pub state: String,
    pub percent: u8,
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
}

struct Job {
    progress: &'static str,
    installed: &'static str,
    opening: &'static AtomicBool,
    windows: &'static str,
    idle_new: &'static str,
    idle_have: &'static str,
    opening_line: &'static str,
    script: fn() -> String,
    ready: fn(&str) -> (String, String),
}

pub fn names_view() -> Status {
    view(&names_job())
}

pub fn door_view() -> Status {
    view(&door_job())
}

pub fn names_start() -> Result<Status, String> {
    start(&names_job())
}

pub fn door_start() -> Result<Status, String> {
    start(&door_job())
}

pub fn house_address(ip: &str) -> bool {
    let Ok(ip) = ip.parse::<Ipv4Addr>() else {
        return false;
    };
    if ip.is_loopback() || ip.is_unspecified() || ip.is_broadcast() || ip.is_link_local() {
        return false;
    }
    let [a, b, _, _] = ip.octets();
    !(a == 100 && (64..128).contains(&b))
}

fn names_job() -> Job {
    Job {
        progress: "/run/keel/names.json",
        installed: "/var/lib/technitium/installed",
        opening: &NAMES_OPENING,
        windows: "Names install on the Keel computer.",
        idle_new: "Keel is opening Names. The first time downloads the program.",
        idle_have: "Names are on this computer. Keel is starting them.",
        opening_line: "Opening Names.",
        script: names_script,
        ready: names_ready,
    }
}

fn door_job() -> Job {
    Job {
        progress: "/run/keel/door.json",
        installed: "/var/lib/keel/caddy/installed",
        opening: &DOOR_OPENING,
        windows: "The front door installs on the Keel computer.",
        idle_new: "Keel is opening the front door. The first time downloads the program.",
        idle_have: "The front door is on this computer. Keel is starting it.",
        opening_line: "Opening the front door.",
        script: door_script,
        ready: door_ready,
    }
}

pub(crate) fn names_script() -> String {
    include_str!("names-dns.sh").to_string()
}

pub(crate) fn door_script() -> String {
    include_str!("names-door.sh").replace("__KEEL_DOOR_HELPER__\n", DOOR_HELPER)
}

/// The front door's helper: it writes the Caddyfile from the sites list and
/// reloads the door. Written when the door is set up and with each name.
const DOOR_HELPER: &str = include_str!("door-helper.sh");

pub fn prepared() -> Result<(), String> {
    if !names_view().ready {
        return Err("Names are still opening. Apps stay closed until Names and the front door are ready.".into());
    }
    if !door_view().ready {
        return Err("The front door is still opening. Apps stay closed until it is ready.".into());
    }
    Ok(())
}

/// Names and the front door do not need the name you own. Keel starts them
/// on its own. The name can be saved or changed at any time.
pub fn open_shared() {
    if !cfg!(target_os = "linux") {
        return;
    }
    std::thread::spawn(|| {
        bring_up(&names_job());
        bring_up(&door_job());
    });
}

fn bring_up(job: &Job) {
    let name = if job.progress.ends_with("names.json") {
        "keel-names"
    } else {
        "keel-door"
    };
    // A saved "ready" note can outlive the program. Start it again when it has stopped.
    if view(job).ready && running(name) {
        return;
    }
    let _ = start(job);
    while view(job).state == "running" {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

pub fn publish(leaf: &str, forward: &str, port: u16) -> Result<bool, String> {
    let owned = crate::owned::load().ok_or("Type the name you own, then press Open.")?;
    if port == 0 || !token_ok(leaf) || !token_ok(forward) {
        return Err("Keel could not write the name.".into());
    }
    let host = format!("{leaf}.{owned}");
    let script = link_script(&owned, &host, forward, port);
    match crate::storage::host_text("exec /bin/bash -s", &[], Some(&script))?.as_str() {
        "locked" => Ok(false),
        "waiting" => Ok(true),
        _ => Err("Keel could not tell whether the lock is ready.".into()),
    }
}

pub fn lock_note(waiting: bool) -> &'static str {
    if waiting {
        " The lock is not ready yet. At the place you bought the name, send it to this computer once."
    } else {
        ""
    }
}

pub(crate) fn link_script(owned: &str, host: &str, forward: &str, port: u16) -> String {
    let mut script = include_str!("name-link.sh").to_string();
    script = script.replace("__KEEL_OWNED__", owned);
    script = script.replace("__KEEL_HOST__", host);
    script = script.replace("__KEEL_FORWARD__", forward);
    script = script.replace("__KEEL_PORT__", &port.to_string());
    script = script.replace("__KEEL_DOOR_HELPER__\n", DOOR_HELPER);
    script
}

fn token_ok(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 63
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn names_ready(_ip: &str) -> (String, String) {
    (
        String::new(),
        format!(
            "Names are on this computer. Keel writes each app name. You do not type addresses."
        ),
    )
}

fn door_ready(_ip: &str) -> (String, String) {
    (
        String::new(),
        format!(
            "The front door is on this computer. Keel sends each app name through it and asks for the lock. This page stays on its own address. People on the public internet also need the router to send ports 80 and 443 to this computer."
        ),
    )
}

fn view(job: &Job) -> Status {
    if !cfg!(target_os = "linux") {
        return Status {
            ready: false,
            address: String::new(),
            message: job.windows.into(),
            state: "unavailable".into(),
            percent: 0,
        };
    }
    if let Some(saved) = read_progress(job.progress) {
        if saved.state == "running" && !job.opening.load(Ordering::SeqCst) {
            return idle(job);
        }
        if saved.state == "running" || saved.state == "error" || saved.ready {
            return Status {
                ready: saved.ready,
                address: saved.address,
                message: saved.message,
                state: saved.state,
                percent: saved.percent,
            };
        }
    }
    idle(job)
}

fn start(job: &Job) -> Result<Status, String> {
    if !cfg!(target_os = "linux") {
        return Err(job.windows.into());
    }
    if job
        .opening
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(view(job));
    }
    write_progress(job.progress, "running", 2, job.opening_line, "", false);
    let progress = job.progress.to_string();
    let opening = job.opening;
    let script = (job.script)();
    let ready = job.ready;
    std::thread::spawn(move || {
        let result = crate::storage::host_text("exec /bin/bash -s", &[], Some(&script));
        match result {
            Ok(ip) if house_address(&ip) => {
                let (address, message) = ready(&ip);
                write_progress(&progress, "done", 100, &message, &address, true);
            }
            Ok(_) => write_progress(
                &progress,
                "error",
                0,
                "Keel could not see the house network, so this did not start.",
                "",
                false,
            ),
            Err(message) => write_progress(&progress, "error", 0, &message, "", false),
        }
        opening.store(false, Ordering::SeqCst);
    });
    Ok(view(job))
}

fn running(name: &str) -> bool {
    let (script, want) = if name == "keel-door" {
        (
            "systemctl is-active --quiet keel-door && systemctl show -p ExecStart keel-door | grep -q /usr/local/bin/caddy && echo yes".to_string(),
            "yes",
        )
    } else {
        (
            format!("podman inspect -f '{{{{.State.Running}}}}' {name} 2>/dev/null"),
            "true",
        )
    };
    crate::storage::host_text("exec /bin/bash -s", &[], Some(&script))
        .ok()
        .as_deref()
        == Some(want)
}

fn idle(job: &Job) -> Status {
    let installed = Path::new(job.installed).is_file();
    Status {
        ready: false,
        address: String::new(),
        message: if installed { job.idle_have } else { job.idle_new }.into(),
        state: "idle".into(),
        percent: 0,
    }
}

fn read_progress(path: &str) -> Option<Saved> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_progress(path: &str, state: &str, percent: u8, message: &str, address: &str, ready: bool) {
    let saved = Saved {
        state: state.into(),
        percent,
        message: message.into(),
        address: address.into(),
        ready,
    };
    let Ok(body) = serde_json::to_vec(&saved) else {
        return;
    };
    let _ = fs::create_dir_all("/run/keel");
    let tmp = format!("{path}.tmp");
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

#[cfg(test)]
mod tests {
    use super::{door_script, house_address, link_script, names_script};

    #[test]
    fn the_house_address_is_not_away_or_this_screen() {
        assert!(house_address("192.168.0.127"));
        assert!(house_address("10.1.2.3"));
        assert!(!house_address("100.113.90.33"));
        assert!(!house_address("127.0.0.1"));
        assert!(!house_address("169.254.1.1"));
        assert!(!house_address("keel"));
    }

    #[test]
    fn names_use_the_pinned_program_on_the_house_address() {
        let script = names_script();
        assert!(script.contains("docker.io/technitium/dns-server:15.6.2"));
        assert!(script.contains("--network host"));
        assert!(script.contains("DNS_SERVER_WEB_SERVICE_LOCAL_ADDRESSES=0.0.0.0"));
        assert!(script.contains("webServiceLocalAddresses"));
        assert!(script.contains("dnsServerLocalEndPoints"));
        assert!(script.contains("systemctl restart keel-names"));
        assert!(script.contains("systemctl start keel-names"));
        assert!(script.contains("http://127.0.0.1:5380/"));
        assert!(script.contains("http://$house:5380/"));
        assert!(script.contains("port_ok \"$house\" 53"));
        assert!(script.contains("rm -f \"$cfg/installed\""));
        assert!(!script.contains("-p "));
        assert!(!script.contains("9478"));
    }

    #[test]
    fn the_front_door_does_not_sit_in_front_of_this_page() {
        let script = door_script();
        assert!(script.contains("ver=2.11.9"));
        assert!(script.contains("caddy_${ver}_linux_arm64.tar.gz"));
        assert!(script.contains("caddy_${ver}_linux_amd64.tar.gz"));
        assert!(script.contains("sha512sum -c"));
        assert!(script.contains("/usr/local/bin/caddy"));
        assert!(script.contains("systemctl restart keel-door"));
        assert!(script.contains("http://127.0.0.1/"));
        assert!(script.contains("http://$house/"));
        assert!(script.contains("bind 127.0.0.1"));
        assert!(script.contains("admin 127.0.0.1:2019"));
        assert!(script.contains("podman rm -f keel-door"));
        assert!(script.contains("caddy-reload.sh"));
        assert!(script.contains("reverse_proxy"));
        let stop = script.find("systemctl stop keel-door.service").expect("stop the old door");
        let write = script.find("/var/lib/keel/caddy-reload.sh >").expect("write the door");
        assert!(stop < write);
        assert!(script.contains("systemctl show -p ExecStart keel-door | grep -q /usr/local/bin/caddy"));
        assert!(script.contains("not enough space"));
        assert!(script.contains("caddy validate --config"));
        assert!(script.contains("address already in use"));
        assert!(!script.contains("fail \"${last"));
        assert!(!script.contains('\r'));
        assert!(!script.contains("DNAT"));
        assert!(!script.contains("-p 80:80"));
        assert!(!script.contains("--network host"));
        assert!(!script.contains("0.0.0.0"));
        assert!(!script.contains("9478"));
        assert!(!script.contains("proxy_pass"));
        assert!(!script.contains(":81"));
    }

    #[test]
    fn one_link_writes_the_name_and_asks_for_the_lock() {
        let script = link_script("klaingar.com", "photos.klaingar.com", "keel-photos-server", 2283);
        assert!(script.contains("owned='klaingar.com'"));
        assert!(script.contains("host='photos.klaingar.com'"));
        assert!(script.contains("forward='keel-photos-server'"));
        assert!(script.contains("port='2283'"));
        assert!(script.contains("http://127.0.0.1:5380/api/zones/records/add"));
        assert!(script.contains("caddy-reload.sh"));
        assert!(script.contains("keel-jitsi-web) upstream=127.0.0.1:8080"));
        assert!(script.contains("keel-passwords) upstream=127.0.0.1:8081"));
        assert!(script.contains("127.0.0.1:$port"));
        assert!(!script.contains("/api/nginx/proxy-hosts"));
        assert!(!script.contains('\r'));
        assert!(!script.contains("9478"));
        assert!(!script.contains("0.0.0.0"));
        assert!(!script.contains("__KEEL_"));
    }

    #[test]
    fn video_takes_recordings_at_one_path_and_nothing_else_moves() {
        let link = link_script("klaingar.com", "video.klaingar.com", "keel-jitsi-web", 8080);
        assert!(link.contains(r#"[ "$forward" = keel-jitsi-web ] && extra=" rec""#));
        assert!(link.contains("cat > /var/lib/keel/caddy-reload.sh.new"));
        let helper = super::DOOR_HELPER;
        assert!(helper.contains("reverse_proxy /keel-recording/* 127.0.0.1:9477"));
        assert!(!helper.contains("9478"));
        assert!(super::door_script().contains("reverse_proxy /keel-recording/* 127.0.0.1:9477"));
        assert!(!super::door_script().contains("__KEEL_"));
    }
}
