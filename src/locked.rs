//! The locked page is this same page at the Away name, with a lock the browser trusts.
//!
//! Keel serves that lock itself. Nothing accepts the connection and forwards it,
//! because a forwarder on this computer would look like the Keel screen.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

static GO: Notify = Notify::const_new();
static UP: AtomicBool = AtomicBool::new(false);
const FLAG: &str = "locked.on";

pub fn address(name: Option<&str>) -> Option<String> {
    let name = name?;
    if wanted() && safe_name(name) {
        Some(format!("https://{name}"))
    } else {
        None
    }
}

pub fn kick() {
    GO.notify_one();
}

pub async fn serve(app: axum::Router) {
    loop {
        if wanted() && !UP.load(Ordering::Relaxed) {
            if let Err(error) = listen(app.clone()).await {
                eprintln!("keel: {error}");
            }
        }
        GO.notified().await;
    }
}

pub fn lock() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("This lock is on the Keel computer.".into());
    }
    let status = crate::overlay::status();
    let name = status
        .dns_name
        .as_deref()
        .filter(|name| safe_name(name))
        .ok_or_else(|| "Connect Away first. The locked page uses that name.".to_string())?;
    let dir = data_dir()?;
    let cert = dir.join("page.crt");
    let key = dir.join("page.key");
    let cert_s = path_arg(&cert)?;
    let key_s = path_arg(&key)?;
    let script = cert_script(&cert_s, &key_s, name);
    crate::storage::host("exec /bin/bash -s", &[], Some(&script))?;
    remember(&dir)?;
    Ok(format!(
        "The locked page is https://{name}. Open that from a phone that is on Away. The Keel screen stays as it is."
    ))
}

pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.ends_with(".ts.net")
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn wanted() -> bool {
    data_dir().ok().is_some_and(|dir| dir.join(FLAG).is_file())
}

fn data_dir() -> Result<PathBuf, String> {
    let raw = std::env::var("KEEL_DATA").unwrap_or_else(|_| "/var/lib/keel".into());
    let path = PathBuf::from(&raw);
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|_| "Keel could not store the lock.".to_string())?
            .join(path)
    };
    path_arg(&path)?;
    Ok(path)
}

fn path_arg(path: &Path) -> Result<String, String> {
    let text = path.to_str().unwrap_or("");
    if text.starts_with('/')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '-' || c == '_')
    {
        Ok(text.to_string())
    } else {
        Err("Keel could not store the lock.".into())
    }
}

fn cert_script(cert: &str, key: &str, name: &str) -> String {
    let dir = Path::new(cert)
        .parent()
        .and_then(|path| path.to_str())
        .unwrap_or("/var/lib/keel");
    format!(
        r#"set -uo pipefail
fail() {{ echo "$1" >&2; exit 1; }}
command -v tailscale >/dev/null 2>&1 || fail "Install Away first."
install -d -m 0755 {dir}
tailscale cert --cert-file {cert} --key-file {key} {name} >/dev/null || fail "Away could not lock the page. Turn on HTTPS certificates for this Away network, then try again."
chmod 0644 {cert} || fail "Keel locked the page but could not store it."
chmod 0600 {key} || fail "Keel locked the page but could not store it."
"#
    )
}

fn cert_is_fresh(dir: &Path) -> bool {
    let Ok(meta) = fs::metadata(dir.join("page.crt")) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    modified
        .elapsed()
        .map(|age| age < std::time::Duration::from_secs(60 * 60))
        .unwrap_or(false)
}

fn remember(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|_| "Keel locked the page but could not remember it.".to_string())?;
    fs::write(dir.join(FLAG), b"on\n").map_err(|_| "Keel locked the page but could not remember it.".to_string())
}

async fn listen(app: axum::Router) -> Result<(), String> {
    let dir = data_dir()?;
    if !cert_is_fresh(&dir) {
        let _ = tokio::task::spawn_blocking(lock).await;
    }
    let cert = dir.join("page.crt");
    let key = dir.join("page.key");
    if !cert.is_file() || !key.is_file() {
        return Err("The locked page has no certificate yet.".into());
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert, &key)
        .await
        .map_err(|_| "Keel could not read the lock.".to_string())?;
    let addr: std::net::SocketAddr = "0.0.0.0:443"
        .parse()
        .map_err(|_| "The locked page could not start.".to_string())?;
    UP.store(true, Ordering::Relaxed);
    let started = axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await;
    UP.store(false, Ordering::Relaxed);
    started.map_err(|_| "The locked page stopped.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_away_name_can_be_locked() {
        assert!(safe_name("keel.tailnet.ts.net"));
        assert!(!safe_name("keel.local"));
        assert!(!safe_name("keel.tailnet.ts.net;rm"));
        assert!(!safe_name(""));
        let script = cert_script(
            "/var/lib/keel/page.crt",
            "/var/lib/keel/page.key",
            "keel.tailnet.ts.net",
        );
        assert!(script.contains("tailscale cert --cert-file /var/lib/keel/page.crt"));
        assert!(script.contains("keel.tailnet.ts.net"));
        assert!(!script.contains("tailscale serve"));
        assert!(!script.contains("proxy"));
    }
}
