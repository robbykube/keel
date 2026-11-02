//! The name a person owns, such as klaingar.com.
//!
//! Keel makes photos.klaingar.com, video.klaingar.com, and the same for each
//! other app, then asks for the lock. The person does not match a name to an
//! address.

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize)]
pub struct SaveRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct Owned {
    pub name: String,
    pub photos: String,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    name: String,
}

pub fn view() -> Owned {
    match load() {
        Some(name) => Owned {
            photos: format!("https://photos.{name}"),
            name,
        },
        None => Owned {
            name: String::new(),
            photos: String::new(),
        },
    }
}

pub fn save(raw: &str) -> Result<Owned, String> {
    let name = normalize(raw)?;
    let saved = Saved { name: name.clone() };
    let body = serde_json::to_vec(&saved).map_err(|_| "Keel could not store the name.".to_string())?;
    let path = path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "Keel could not store the name.".to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body).map_err(|_| "Keel could not store the name.".to_string())?;
    std::fs::rename(&tmp, path).map_err(|_| "Keel could not store the name.".to_string())?;
    Ok(Owned {
        photos: format!("https://photos.{name}"),
        name,
    })
}

pub fn load() -> Option<String> {
    let bytes = std::fs::read(path()).ok()?;
    let saved: Saved = serde_json::from_slice(&bytes).ok()?;
    normalize(&saved.name).ok()
}

pub fn photos_address(owned: &str) -> Result<String, String> {
    app_address(owned, "photos")
}

pub fn app_address(owned: &str, leaf: &str) -> Result<String, String> {
    let name = normalize(owned)?;
    if !label_ok(leaf) {
        return Err("Type the name you own, then press Open.".into());
    }
    Ok(format!("https://{leaf}.{name}"))
}

pub fn normalize(raw: &str) -> Result<String, String> {
    let name = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if name.is_empty()
        || name.contains("://")
        || name.contains('/')
        || name.contains(':')
        || name.contains(' ')
        || name.contains('@')
    {
        return Err("Type the name only, such as klaingar.com.".into());
    }
    let labels: Vec<&str> = name.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|label| !label_ok(label)) {
        return Err("Type the name only, such as klaingar.com.".into());
    }
    if name.ends_with(".ts.net") || name.parse::<std::net::Ipv4Addr>().is_ok() {
        return Err("Type the name you bought, such as klaingar.com.".into());
    }
    Ok(name)
}

fn label_ok(label: &str) -> bool {
    let bytes = label.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 63
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-')
}

fn path() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join("owned-name.json")
}

#[cfg(test)]
mod tests {
    use super::{normalize, photos_address};

    #[test]
    fn a_bought_name_becomes_the_photos_address() {
        assert_eq!(normalize("Klaingar.com").as_deref(), Ok("klaingar.com"));
        assert_eq!(normalize("klaingar.com.").as_deref(), Ok("klaingar.com"));
        assert_eq!(
            photos_address("klaingar.com").as_deref(),
            Ok("https://photos.klaingar.com")
        );
    }

    #[test]
    fn an_address_or_a_path_is_refused() {
        assert!(normalize("https://klaingar.com").is_err());
        assert!(normalize("klaingar.com/photos").is_err());
        assert!(normalize("photos.klaingar.com:443").is_err());
        assert!(normalize("192.168.0.127").is_err());
        assert!(normalize("keel.example.ts.net").is_err());
        assert!(normalize("klaingar").is_err());
        assert!(normalize("").is_err());
    }
}
