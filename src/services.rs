use serde::Serialize;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

use crate::model::RepairResult;

const CORE_UNITS: &[(&str, &str)] = &[
    ("keel.service", "control plane"),
    ("tailscaled.service", "overlay client"),
    ("smbd.service", "file shares"),
];

#[derive(Serialize)]
pub struct ServiceStatus {
    pub unit: &'static str,
    pub role: &'static str,
    pub active: String,
    pub enabled: String,
    pub evidence: String,
}

#[derive(Serialize)]
pub struct PortStatus {
    pub port: u16,
    pub role: &'static str,
    pub listening: bool,
    pub evidence: String,
}

pub fn core_status() -> Vec<ServiceStatus> {
    CORE_UNITS
        .iter()
        .map(|(unit, role)| {
            if cfg!(target_os = "linux") {
                linux_status(unit, role)
            } else {
                ServiceStatus {
                    unit,
                    role,
                    active: "unavailable".into(),
                    enabled: "unavailable".into(),
                    evidence: "systemd is only available on the Linux Keel appliance".into(),
                }
            }
        })
        .collect()
}

pub fn port_status() -> Vec<PortStatus> {
    [
        (9478, "Keel GUI"),
        (22, "SSH recovery"),
        (445, "SMB shares"),
    ]
    .into_iter()
    .map(|(port, role)| {
        if cfg!(target_os = "linux") || cfg!(target_os = "windows") {
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            let listening = TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok();
            PortStatus {
                port,
                role,
                listening,
                evidence: format!("tcp://127.0.0.1:{port}"),
            }
        } else {
            PortStatus {
                port,
                role,
                listening: false,
                evidence: "port probe unavailable on this host".into(),
            }
        }
    })
    .collect()
}

pub fn restart(id: &str) -> RepairResult {
    let unit = match id {
        "restart-keel" => "keel.service",
        "restart-samba" => "smbd.service",
        "restart-tailscale" => "tailscaled.service",
        _ => {
            return RepairResult {
                ok: false,
                id: id.into(),
                message: "That service is not in Keel's safe restart allowlist.".into(),
            }
        }
    };

    if !cfg!(target_os = "linux") {
        return RepairResult {
            ok: false,
            id: id.into(),
            message: "Service restart is available on the Linux Keel appliance only.".into(),
        };
    }

    match Command::new("systemctl").args(["restart", unit]).status() {
        Ok(status) if status.success() => RepairResult {
            ok: true,
            id: id.into(),
            message: format!("Restarted {unit}; recheck its status."),
        },
        Ok(_) => RepairResult {
            ok: false,
            id: id.into(),
            message: format!("systemd refused to restart {unit}."),
        },
        Err(_) => RepairResult {
            ok: false,
            id: id.into(),
            message: "systemctl is not available on this Linux host.".into(),
        },
    }
}

fn linux_status(unit: &'static str, role: &'static str) -> ServiceStatus {
    let active = systemctl_state(&["is-active", unit]);
    let enabled = systemctl_state(&["is-enabled", unit]);
    let evidence = format!("systemctl is-active={active}; is-enabled={enabled}");
    ServiceStatus {
        unit,
        role,
        active,
        enabled,
        evidence,
    }
}

fn systemctl_state(args: &[&str]) -> String {
    Command::new("systemctl")
        .args(args)
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                stdout
            } else if output.status.code() == Some(1) {
                "inactive".into()
            } else {
                "unavailable".into()
            }
        })
        .unwrap_or_else(|_| "unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::core_status;

    #[test]
    fn exposes_fixed_core_unit_set() {
        let statuses = core_status();
        assert_eq!(statuses.len(), 3);
        assert_eq!(statuses[0].unit, "keel.service");
        assert_eq!(statuses[1].unit, "tailscaled.service");
        assert_eq!(statuses[2].unit, "smbd.service");
    }

    #[test]
    fn rejects_arbitrary_restart_units() {
        let result = super::restart("restart-random-unit");
        assert!(!result.ok);
        assert!(result.message.contains("allowlist"));
    }
}
