use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::overlay::Provider;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub overlay: OverlayConfig,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OverlayConfig {
    #[serde(default)]
    pub provider: Provider,
    #[serde(default)]
    pub headscale_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            overlay: OverlayConfig::default(),
        }
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            provider: Provider::Headscale,
            headscale_url: None,
        }
    }
}

pub fn load() -> Config {
    let path = config_path();
    let mut config = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Config>(&bytes).ok())
        .unwrap_or_default();

    if config.version != CONFIG_VERSION {
        config.version = CONFIG_VERSION;
    }
    if let Ok(provider) = std::env::var("KEEL_OVERLAY_PROVIDER") {
        config.overlay.provider = Provider::from_name(&provider);
    }
    config
}

pub fn config_path() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("config.json")
}

/// Persist configuration without leaving a partially-written JSON file.
/// Mutation endpoints will call this only after authentication and validation.
#[allow(dead_code)]
pub fn save(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::overlay::Provider;

    #[test]
    fn defaults_to_self_hosted_overlay() {
        let config = Config::default();
        assert!(matches!(config.overlay.provider, Provider::Headscale));
        assert_eq!(config.version, 1);
    }
}
