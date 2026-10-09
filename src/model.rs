use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct Health {
    pub ok: bool,
    pub name: &'static str,
    pub version: &'static str,
    pub uptime_ms: u128,
    pub inventory: &'static str,
}

#[derive(Serialize)]
pub struct SystemSnapshot {
    pub hostname: String,
    pub os: String,
    /// "linux" or "windows". `os` is a human string like "Debian GNU/Linux 13",
    /// so the GUI needs this to tell a real appliance from a dev host.
    pub platform: &'static str,
    /// Non-loopback IPv4 addresses. The person sitting at the appliance has no
    /// other way to learn where to reach it from their own computer.
    pub addresses: Vec<String>,
    pub cpu_pct: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub daemon_rss: u64,
    pub cores: u32,
}

impl SystemSnapshot {
    pub fn empty() -> Self {
        Self {
            hostname: "keel".into(),
            os: std::env::consts::OS.into(),
            platform: std::env::consts::OS,
            addresses: Vec::new(),
            cpu_pct: 0.0,
            mem_used: 0,
            mem_total: 0,
            daemon_rss: 0,
            cores: 0,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Disk {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub kind: String,
    pub model: String,
    pub transport: String,
    pub rotational: bool,
    pub mount: String,
    pub fstype: String,
}

#[derive(Serialize)]
pub struct InstallerStatus {
    pub mode: &'static str,
    pub can_install: bool,
    pub network: &'static str,
    pub warning: &'static str,
    pub disks: Vec<Disk>,
}

#[derive(Deserialize)]
pub struct InstallerPlan {
    pub target: String,
    pub confirmation: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Pool {
    pub name: String,
    pub kind: String,
    pub source: String,
    pub size: u64,
    pub used: u64,
}

#[derive(Deserialize)]
pub struct NewPool {
    pub name: String,
    pub kind: String,
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Share {
    pub name: String,
    pub path: String,
    pub protocol: String,
    pub pool: String,
}

#[derive(Serialize, Clone)]
pub struct ShareView {
    pub name: String,
    pub path: String,
    pub protocol: String,
    pub pool: String,
    pub lan: Vec<String>,
    pub overlay: Vec<String>,
    pub username: String,
    pub password: String,
    pub ready: bool,
    pub why: String,
}

#[derive(Deserialize)]
pub struct NewShare {
    pub name: String,
    pub pool: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct Power {
    pub policy: &'static str,
    pub mode: &'static str,
    pub host_sleep_allowed: bool,
    pub services_stopped: bool,
    pub overlay_required: bool,
    pub why: &'static str,
}

#[derive(Serialize)]
pub struct Opportunity {
    pub id: &'static str,
    pub title: &'static str,
    pub why: &'static str,
    pub phase: &'static str,
    pub safe_while_working: bool,
    pub auto: bool,
    pub evidence: String,
}

#[derive(Serialize)]
pub struct Guest {
    pub id: u16,
    pub name: &'static str,
    pub role: &'static str,
    pub floor: bool,
    pub status: &'static str,
    pub why: &'static str,
}

#[derive(Serialize)]
pub struct Issue {
    pub id: &'static str,
    pub severity: &'static str,
    pub title: &'static str,
    pub why: &'static str,
    pub evidence: String,
    pub repair: Option<RepairAction>,
}

#[derive(Serialize, Clone)]
pub struct RepairAction {
    pub id: &'static str,
    pub label: &'static str,
    pub safe: bool,
    pub available: bool,
}

#[derive(Serialize)]
pub struct IssuesReport {
    pub red: bool,
    pub items: Vec<Issue>,
    pub playbook: Vec<RepairAction>,
}

#[derive(Deserialize)]
pub struct RepairRequest {
    pub id: String,
}

#[derive(Serialize)]
pub struct RepairResult {
    pub ok: bool,
    pub id: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub updated_ms: u64,
}

#[derive(Deserialize)]
pub struct NewNote {
    pub title: String,
    pub body: String,
}
