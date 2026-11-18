//! Light or full: for an app with a lighter program, which one runs on this
//! computer. Keel suggests one from the computer's memory; the person can
//! pick the other in Settings. The app keeps its name either way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Below this much memory, Keel suggests the light program.
const LIGHT_BELOW_MB: u64 = 2560;

struct App {
    id: &'static str,
    title: &'static str,
    light: &'static str,
    full: &'static str,
    light_mb: u64,
    full_mb: u64,
    /// What the light program gives, in a few words.
    light_is: &'static str,
    full_is: &'static str,
}

const APPS: &[App] = &[App {
    id: "video",
    title: "Video",
    light: "Galène",
    full: "Jitsi",
    light_mb: 150,
    full_mb: crate::memory::NEEDS_VIDEO,
    light_is: "Calls in the browser, recording included. One small program.",
    full_is: "Jitsi Meet, with its phone apps. Four programs, two of them Java.",
}];

/// Where Keel keeps Galène, from the Keel package.
pub const GALENE: &str = "/usr/lib/keel/apps/galene/galene";

#[derive(Serialize)]
pub struct Choice {
    pub app: &'static str,
    pub title: &'static str,
    pub light: &'static str,
    pub full: &'static str,
    pub light_mb: u64,
    pub full_mb: u64,
    pub light_is: &'static str,
    pub full_is: &'static str,
    /// "light" or "full".
    pub chosen: String,
    pub suggested: &'static str,
    /// Whether the light program is on this computer yet.
    pub light_ready: bool,
    pub why: String,
}

fn path() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join("app-versions.json")
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    #[serde(flatten)]
    chosen: BTreeMap<String, String>,
}

fn saved() -> BTreeMap<String, String> {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice::<Saved>(&b).ok()).map(|s| s.chosen).unwrap_or_default()
}

fn total_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

fn suggest(total_mb: u64) -> &'static str {
    if total_mb > 0 && total_mb < LIGHT_BELOW_MB {
        "light"
    } else {
        "full"
    }
}

fn light_ready(app: &str) -> bool {
    match app {
        "video" => Path::new(GALENE).is_file() || std::env::var_os("KEEL_GALENE").is_some(),
        _ => false,
    }
}

fn why(total_mb: u64, app: &App, suggested: &str) -> String {
    let gb = total_mb as f64 / 1024.0;
    if suggested == "light" {
        format!(
            "This computer has {gb:.1} GB of memory. {} needs about {} MB; {} needs about {} MB.",
            app.light, app.light_mb, app.full, app.full_mb
        )
    } else {
        format!("This computer has {gb:.1} GB of memory, enough for {}.", app.full)
    }
}

pub fn list() -> Vec<Choice> {
    let total = total_mb();
    let saved = saved();
    APPS.iter()
        .map(|app| {
            let suggested = suggest(total);
            Choice {
                app: app.id,
                title: app.title,
                light: app.light,
                full: app.full,
                light_mb: app.light_mb,
                full_mb: app.full_mb,
                light_is: app.light_is,
                full_is: app.full_is,
                chosen: saved.get(app.id).cloned().unwrap_or_else(|| suggested.to_string()),
                suggested,
                light_ready: light_ready(app.id),
                why: why(total, app, suggested),
            }
        })
        .collect()
}

/// Whether `app` runs its light program here. The light one is used only
/// when it is on this computer; until then the full one stays.
pub fn light(app: &str) -> bool {
    let chosen = saved().get(app).cloned().unwrap_or_else(|| suggest(total_mb()).to_string());
    chosen == "light" && light_ready(app)
}

/// Keeps a choice. Video that is open is closed first, so the two programs
/// never hold the meeting address at once.
pub fn choose(app: &str, version: &str) -> Result<String, String> {
    let known = APPS.iter().find(|a| a.id == app).ok_or("Keel does not know that app.")?;
    if version != "light" && version != "full" {
        return Err("Pick light or full.".into());
    }
    if version == "light" && !light_ready(app) {
        return Err(format!("{} comes with the next Keel update.", known.light));
    }
    let before = light(app);
    if app == "video" && before != (version == "light") {
        if before {
            let _ = crate::galene::close();
        } else {
            let _ = crate::jitsi::close();
        }
    }
    let mut all = saved();
    all.insert(app.into(), version.into());
    let body = serde_json::to_vec(&Saved { chosen: all }).map_err(|_| "Keel could not keep that.".to_string())?;
    let file = path();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&file, body).map_err(|_| "Keel could not keep that.".to_string())?;
    let name = if version == "light" { known.light } else { known.full };
    Ok(format!("{} now uses {name}. Press Open on Apps when you want it.", known.title))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_small_computer_is_suggested_the_light_program() {
        assert_eq!(super::suggest(914), "light");
        assert_eq!(super::suggest(3900), "full");
        assert_eq!(super::suggest(0), "full");
        let video = &super::APPS[0];
        let said = super::why(914, video, "light");
        assert!(said.contains("0.9 GB"), "{said}");
        assert!(said.contains("Galène needs about 150 MB"), "{said}");
    }
}
