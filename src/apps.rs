use serde::Deserialize;
use serde::Serialize;
use std::process::Command;

use crate::model::RepairResult;

#[derive(Serialize)]
pub struct AppManifest {
    pub manifest_version: u8,
    pub id: &'static str,
    pub name: &'static str,
    pub role: &'static str,
    pub runtime: &'static str,
    pub status: &'static str,
    pub floor: bool,
    pub ports: &'static [u16],
    pub dependencies: &'static [&'static str],
    pub data_paths: &'static [&'static str],
    pub healthcheck: &'static str,
    pub idle_policy: &'static str,
    pub why: &'static str,
}

#[derive(Deserialize)]
pub struct ActionRequest {
    pub action: String,
}

pub fn manifests(idle: bool) -> Vec<AppManifest> {
    let definitions: &[(
        &str,
        &str,
        &str,
        bool,
        &[u16],
        &[&str],
        &[&str],
        &str,
        &str,
        &str,
    )] = &[
        (
            "tailscale",
            "Overlay",
            "native",
            true,
            &[],
            &[],
            &["/var/lib/tailscale"],
            "tailscale status",
            "floor",
            "The private path to the GUI and files away from home.",
        ),
        (
            "scanopy",
            "Analytics",
            "native",
            false,
            &[],
            &[],
            &["/var/lib/scanopy"],
            "systemd",
            "park",
            "Installed for the curated Keel catalog; closed until requested.",
        ),
        (
            "technitium",
            "DNS",
            "native",
            false,
            &[53, 5380],
            &[],
            &["/var/lib/technitium"],
            "http",
            "keep_warm",
            "Local names should remain available when the box is active.",
        ),
        (
            "vaultwarden",
            "Passwords",
            "container",
            false,
            &[80, 3012],
            &[],
            &["/var/lib/vaultwarden"],
            "http",
            "keep_warm",
            "A password service with persistent, explicitly backed-up data.",
        ),
        (
            "opencloud",
            "Files",
            "container",
            false,
            &[9200],
            &["samba"],
            &["/var/lib/opencloud"],
            "http",
            "keep_warm",
            "A file app layered over Keel storage; SMB remains a core OS path.",
        ),
        (
            "jitsi",
            "Video",
            "container",
            false,
            &[443, 10000],
            &[],
            &["/var/lib/jitsi"],
            "http",
            "on_demand",
            "Video is expensive at idle; start it for a meeting.",
        ),
        (
            "immich",
            "Photos",
            "container",
            false,
            &[2283],
            &["database"],
            &["/var/lib/immich"],
            "http",
            "on_demand",
            "Photo browsing can be warm while heavy workers sleep between jobs.",
        ),
        (
            "pbs",
            "Backup",
            "native",
            false,
            &[8007],
            &[],
            &["/var/lib/pbs"],
            "http",
            "scheduled",
            "Wake only for a declared backup window.",
        ),
        (
            "npm",
            "Proxy",
            "container",
            false,
            &[80, 443, 81],
            &[],
            &["/var/lib/npm"],
            "http",
            "keep_warm",
            "The local front door for apps that are meant to be reachable.",
        ),
        (
            "papra",
            "Documents",
            "container",
            false,
            &[1221],
            &[],
            &["/var/lib/papra"],
            "http",
            "on_demand",
            "A curated document app that need not consume resources while closed.",
        ),
        (
            "mail-gateway",
            "Mail",
            "container",
            false,
            &[25, 587],
            &[],
            &["/var/lib/mail-gateway"],
            "tcp",
            "scheduled",
            "Mail is an explicit workload with a declared wake policy.",
        ),
        (
            "open-archiver",
            "Archive",
            "container",
            false,
            &[8080],
            &[],
            &["/var/lib/open-archiver"],
            "http",
            "on_demand",
            "Archive work is started when needed and backed up as data.",
        ),
    ];

    definitions
        .iter()
        .map(
            |(
                id,
                role,
                runtime,
                floor,
                ports,
                dependencies,
                data_paths,
                healthcheck,
                idle_policy,
                why,
            )| AppManifest {
                manifest_version: 1,
                id,
                name: *id,
                role,
                runtime,
                status: if *floor {
                    "running"
                } else if idle {
                    "stopped"
                } else if *idle_policy == "on_demand" || *idle_policy == "scheduled" {
                    "parked"
                } else {
                    "running"
                },
                floor: *floor,
                ports,
                dependencies,
                data_paths,
                healthcheck,
                idle_policy,
                why,
            },
        )
        .collect()
}

pub fn action(id: &str, requested: &str) -> RepairResult {
    if !manifests(true).iter().any(|app| app.id == id) {
        return RepairResult {
            ok: false,
            id: id.into(),
            message: "That app is not in Keel's catalog.".into(),
        };
    }
    let command = match requested {
        "start" => "start",
        "stop" | "park" => "stop",
        "restart" => "restart",
        _ => {
            return RepairResult {
                ok: false,
                id: id.into(),
                message: "Supported app actions are start, stop, restart, and park.".into(),
            }
        }
    };
    if !cfg!(target_os = "linux") {
        return RepairResult {
            ok: false,
            id: id.into(),
            message: "App lifecycle is available on the Linux Keel appliance only.".into(),
        };
    }

    let unit = format!("keel-app@{id}.service");
    match Command::new("systemctl").args([command, &unit]).status() {
        Ok(status) if status.success() => RepairResult {
            ok: true,
            id: id.into(),
            message: format!("App action `{requested}` completed; recheck health."),
        },
        Ok(_) => RepairResult {
            ok: false,
            id: id.into(),
            message: format!("systemd refused app action `{requested}` for {id}."),
        },
        Err(_) => RepairResult {
            ok: false,
            id: id.into(),
            message: "systemctl is not available on this Linux host.".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::manifests;

    #[test]
    fn every_manifest_has_a_runtime_and_policy() {
        let apps = manifests(true);
        assert_eq!(apps.len(), 12);
        assert!(apps
            .iter()
            .all(|app| !app.runtime.is_empty() && !app.idle_policy.is_empty()));
        assert!(apps.iter().any(|app| app.id == "tailscale" && app.floor));
    }

    #[test]
    fn lifecycle_rejects_unknown_app_and_action() {
        assert!(!super::action("not-in-catalog", "start").ok);
        assert!(!super::action("immich", "destroy").ok);
    }
}
