use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

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
    pub choosable: bool,
    pub installed: bool,
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
            false,
            &[],
            &[],
            &["/var/lib/tailscale"],
            "tailscale status",
            "floor",
            "The private path to the GUI and files away from home.",
        ),
        (
            "technitium",
            "DNS",
            "native",
            false,
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
            true,
            &[80, 3012],
            &[],
            &["/var/lib/vaultwarden"],
            "http",
            "keep_warm",
            "Keep the passwords for this house.",
        ),
        (
            "jitsi",
            "Video",
            "container",
            false,
            true,
            &[443, 10000],
            &[],
            &["/var/lib/jitsi"],
            "http",
            "on_demand",
            "Start a meeting when you need one.",
        ),
        (
            "immich",
            "Photos",
            "container",
            false,
            true,
            &[2283],
            &["database"],
            &["/var/lib/immich"],
            "http",
            "on_demand",
            "Show the pictures that are already in Media.",
        ),
        (
            "npm",
            "Proxy",
            "container",
            false,
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
            "Papers",
            "container",
            false,
            true,
            &[1221],
            &[],
            &["/var/lib/papra"],
            "http",
            "on_demand",
            "File papers here. The Documents folder stays the files themselves.",
        ),
        (
            "mail-gateway",
            "Mail",
            "container",
            false,
            true,
            &[25, 587],
            &[],
            &["/var/lib/mail-gateway"],
            "tcp",
            "scheduled",
            "Send and receive mail.",
        ),
        (
            "open-archiver",
            "Archive",
            "container",
            false,
            true,
            &[8080],
            &[],
            &["/var/lib/open-archiver"],
            "http",
            "on_demand",
            "Keep an old copy of mail and files.",
        ),
    ];

    let chosen = chosen_ids();
    definitions
        .iter()
        .map(
            |(
                id,
                role,
                runtime,
                floor,
                choosable,
                ports,
                dependencies,
                data_paths,
                healthcheck,
                idle_policy,
                why,
            )| AppManifest {
                manifest_version: 1,
                id,
                name: if *choosable { *role } else { *id },
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
                choosable: *choosable,
                installed: chosen.contains(*id),
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

#[derive(Deserialize)]
pub struct ChooseRequest {
    pub ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct ChosenFile {
    ids: Vec<String>,
}

const CHOOSABLE: &[&str] = &[
    "vaultwarden",
    "jitsi",
    "immich",
    "papra",
    "mail-gateway",
    "open-archiver",
];

fn lock_choices() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn chosen_path() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join("chosen-apps.json")
}

pub fn chosen_ids() -> BTreeSet<String> {
    let _lock = lock_choices();
    read_chosen()
}

fn read_chosen() -> BTreeSet<String> {
    let Ok(bytes) = std::fs::read(chosen_path()) else {
        return BTreeSet::new();
    };
    serde_json::from_slice::<ChosenFile>(&bytes)
        .map(|file| file.ids.into_iter().collect())
        .unwrap_or_default()
}

pub fn choose(ids: &[String]) -> Result<Vec<String>, String> {
    if ids.is_empty() {
        return Err("Pick at least one.".into());
    }
    let mut add = Vec::new();
    for id in ids {
        let id = id.trim();
        if !CHOOSABLE.contains(&id) {
            return Err("That one is not on the list.".into());
        }
        if !add.iter().any(|have: &String| have == id) {
            add.push(id.to_string());
        }
    }
    let _lock = lock_choices();
    let path = chosen_path();
    let mut have: Vec<String> = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ChosenFile>(&bytes).ok())
        .map(|file| file.ids)
        .unwrap_or_default();
    for id in add {
        if !have.iter().any(|saved| saved == &id) {
            have.push(id);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "Keel could not save that choice.".to_string())?;
    }
    let body = serde_json::to_vec_pretty(&ChosenFile { ids: have.clone() })
        .map_err(|_| "Keel could not save that choice.".to_string())?;
    std::fs::write(&path, body).map_err(|_| "Keel could not save that choice.".to_string())?;
    Ok(have)
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
        assert_eq!(apps.len(), 9);
        assert!(apps
            .iter()
            .all(|app| app.id != "opencloud" && app.id != "pbs" && app.id != "scanopy"));
        assert!(apps
            .iter()
            .all(|app| !app.runtime.is_empty() && !app.idle_policy.is_empty()));
        assert!(apps.iter().any(|app| app.id == "tailscale" && app.floor && !app.choosable));
        let offered: Vec<_> = apps.iter().filter(|app| app.choosable).map(|app| app.id).collect();
        assert_eq!(offered, super::CHOOSABLE);
    }

    #[test]
    fn first_choice_stays_and_another_can_be_added() {
        let dir = std::env::temp_dir().join(format!("keel-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("KEEL_DATA", &dir);
        let empty = super::choose(&[]);
        assert!(matches!(empty, Err(message) if message.contains("one")));
        let overlay = super::choose(&["tailscale".into()]);
        assert!(matches!(overlay, Err(message) if message.contains("list")));
        assert_eq!(super::choose(&["immich".into()]).unwrap(), vec!["immich".to_string()]);
        assert_eq!(
            super::choose(&["vaultwarden".into()]).unwrap(),
            vec!["immich".to_string(), "vaultwarden".to_string()]
        );
        let apps = manifests(true);
        assert!(apps.iter().any(|app| app.id == "immich" && app.installed));
        assert!(apps.iter().any(|app| app.id == "jitsi" && app.choosable && !app.installed));
        std::env::remove_var("KEEL_DATA");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lifecycle_rejects_unknown_app_and_action() {
        assert!(!super::action("not-in-catalog", "start").ok);
        assert!(!super::action("immich", "destroy").ok);
    }
}
