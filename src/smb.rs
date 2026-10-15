use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sysinfo::Networks;

use crate::model::{Share, ShareView};
use crate::store::Store;

const DEFAULT_SHARE: &str = "files";
const DEFAULT_USER: &str = "keel";
const DEFAULT_PASSWORD: &str = "keel";

pub fn bootstrap(store: &mut Store) {
    let _ = fs::create_dir_all(files_root());
    if cfg!(target_os = "linux") && store.shares.is_empty() {
        store.ensure_default_share(default_share());
    }
    apply(&store.shares);
}

pub fn decorate(shares: &[Share]) -> Vec<ShareView> {
    let lan = lan_uncs();
    let overlay = overlay_uncs();
    let owner = crate::storage::owner();
    let (username, password) = match &owner {
        Some(name) => (name.clone(), String::new()),
        None => (share_username(), share_password()),
    };
    let (ready, mut why) = status_why();
    if ready && owner.is_some() {
        why = "Only the name below and its password open this folder, at home or from Away.".into();
    }
    let hidden = "Hidden until its disk is connected again, so nothing is written to the wrong disk.";
    shares
        .iter()
        .map(|share| (share, available(share)))
        .map(|(share, open)| ShareView {
            name: share.name.clone(),
            path: share.path.clone(),
            protocol: share.protocol.clone(),
            pool: share.pool.clone(),
            lan: lan.iter().map(|ip| unc(ip, &share.name)).collect(),
            overlay: overlay.iter().map(|ip| unc(ip, &share.name)).collect(),
            username: username.clone(),
            password: password.clone(),
            ready: ready && open,
            why: if open { why.clone() } else { hidden.into() },
        })
        .collect()
}

pub fn apply(shares: &[Share]) {
    if !cfg!(target_os = "linux") {
        return;
    }
    let root = files_root();
    let _ = fs::create_dir_all(&root);
    let owner = crate::storage::owner();
    let ids = owner.as_deref().and_then(account_ids);
    let open: Vec<Share> = shares.iter().filter(|s| available(s)).cloned().collect();
    for share in &open {
        let dir = share_dir(share, &root);
        if !dir.exists() && fs::create_dir_all(&dir).is_ok() && !share.read_only {
            give_to(&dir, ids);
        }
    }
    let conf_path = shares_conf_path();
    if let Some(parent) = conf_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = render_shares_conf(&open, &root, owner.as_deref());
    if fs::write(&conf_path, body).is_err() {
        return;
    }
    reload_smbd();
}

pub fn valid_share_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name == name.trim()
        && !name.ends_with('.')
        && !name.contains("  ")
        && name.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' '
        })
}

fn default_share() -> Share {
    Share {
        name: DEFAULT_SHARE.into(),
        path: files_root().to_string_lossy().into(),
        protocol: "smb".into(),
        pool: "this machine".into(),
        read_only: false,
    }
}

fn files_root() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("files")
}

fn shares_conf_path() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("smb-shares.conf")
}

fn share_dir(share: &Share, root: &Path) -> PathBuf {
    let path = share.path.trim();
    if path.is_empty() || path == "/" {
        root.join(&share.name)
    } else if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        root.join(path.trim_start_matches('/'))
    }
}

/// A share on one of Keel's disks is only offered while that disk is open,
/// so nothing is written into the empty mount folder on the system disk.
pub fn available(share: &Share) -> bool {
    match crate::storage::share_role(&share.path) {
        Some(role) => crate::storage::mounted(&role),
        None => true,
    }
}

fn render_shares_conf(shares: &[Share], root: &Path, owner: Option<&str>) -> String {
    let mut out = String::from("# Written by Keel. Do not edit by hand.\n");
    for share in shares {
        if !valid_share_name(&share.name) {
            continue;
        }
        let path = share_dir(share, root);
        let read_only = if share.read_only { "yes" } else { "no" };
        let access = match owner {
            Some(name) => format!(
                "\tguest ok = no\n\
                 \tvalid users = {name}\n\
                 \tforce user = {name}\n\
                 \tforce group = {name}\n\
                 \tcreate mask = 0660\n\
                 \tdirectory mask = 0770\n"
            ),
            None => {
                let user = share_username();
                format!(
                    "\tguest ok = yes\n\
                     \tforce user = {user}\n\
                     \tforce group = {user}\n\
                     \tcreate mask = 0666\n\
                     \tdirectory mask = 0777\n"
                )
            }
        };
        let shown = path.display().to_string().replace('"', "");
        out.push_str(&format!(
            "\n[{}]\n\
             \tpath = \"{shown}\"\n\
             \tbrowseable = yes\n\
             \tread only = {read_only}\n\
             \tveto files = /lost+found/\n\
             {access}",
            share.name,
        ));
    }
    out
}

/// uid and gid of a local account, read from `/etc/passwd`.
fn account_ids(name: &str) -> Option<(u32, u32)> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.len() > 3 && fields[0] == name)
            .then(|| Some((fields[2].parse().ok()?, fields[3].parse().ok()?)))
            .flatten()
    })
}

#[cfg(unix)]
fn give_to(dir: &Path, ids: Option<(u32, u32)>) {
    if let Some((uid, gid)) = ids {
        let _ = std::os::unix::fs::chown(dir, Some(uid), Some(gid));
    }
}

#[cfg(not(unix))]
fn give_to(_dir: &Path, _ids: Option<(u32, u32)>) {}

fn share_username() -> String {
    std::env::var("KEEL_SHARE_USER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_USER.into())
}

fn share_password() -> String {
    if !cfg!(target_os = "linux") {
        return String::new();
    }
    std::env::var("KEEL_SHARE_PASSWORD")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_PASSWORD.into())
}

fn status_why() -> (bool, String) {
    if !cfg!(target_os = "linux") {
        return (
            false,
            "Folder sharing runs on the Linux Keel computer. This Windows copy cannot serve files to File Explorer.".into(),
        );
    }
    if !smbd_present() {
        return (
            false,
            "Samba is not installed on this computer yet. The live image includes it; a bootstrap host needs the samba package.".into(),
        );
    }
    if port_open(445) {
        (
            true,
            "Anyone on this house network can open this folder. That is on purpose until Keel has real sign-in. Use File Explorer; the password is the LAN default shown here.".into(),
        )
    } else {
        (
            false,
            "Keel wrote the folder list, but nothing is listening on port 445 yet. Open Repair and choose Restart SMB.".into(),
        )
    }
}

fn smbd_present() -> bool {
    Command::new("smbd")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(150),
    )
    .is_ok()
}

fn reload_smbd() {
    let _ = Command::new("smbcontrol")
        .args(["smbd", "reload-config"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn lan_uncs() -> Vec<String> {
    advertised_ipv4()
}

fn overlay_uncs() -> Vec<String> {
    crate::overlay::status()
        .overlay_ip
        .into_iter()
        .filter(|ip| !ip.is_empty())
        .collect()
}

fn advertised_ipv4() -> Vec<String> {
    if let Ok(bind) = std::env::var("KEEL_BIND") {
        let bind = bind.trim();
        if !bind.is_empty() && bind != "0.0.0.0" && bind != "::" && bind != "127.0.0.1" {
            return vec![bind.to_string()];
        }
    }
    let networks = Networks::new_with_refreshed_list();
    let mut ips = Vec::new();
    for data in networks.values() {
        for net in data.ip_networks() {
            if let IpAddr::V4(v4) = net.addr {
                if !v4.is_loopback() && !v4.is_link_local() && !v4.is_multicast() {
                    ips.push(v4.to_string());
                }
            }
        }
    }
    ips.sort();
    ips.dedup();
    ips
}

fn unc(host: &str, share: &str) -> String {
    format!(r"\\{host}\{share}")
}

#[cfg(test)]
mod tests {
    use super::{render_shares_conf, valid_share_name, Share};
    use std::path::Path;

    #[test]
    fn share_names_are_plain() {
        assert!(valid_share_name("files"));
        assert!(valid_share_name("photos-1"));
        assert!(valid_share_name("Bills and Taxes"));
        assert!(!valid_share_name(""));
        assert!(!valid_share_name("my  files"));
        assert!(!valid_share_name(" trailing "));
        assert!(!valid_share_name("[oops]"));
    }

    #[test]
    fn writes_guest_writable_stanza() {
        let conf = render_shares_conf(
            &[Share {
                name: "files".into(),
                path: "/var/lib/keel/files".into(),
                protocol: "smb".into(),
                pool: "this machine".into(),
                read_only: false,
            }],
            Path::new("/var/lib/keel/files"),
            None,
        );
        assert!(conf.contains("[files]"));
        assert!(conf.contains("guest ok = yes"));
        assert!(conf.contains("read only = no"));
        assert!(conf.contains("path = \"/var/lib/keel/files\""));
    }

    #[test]
    fn owner_closes_guest_access_and_copies_are_read_only() {
        let conf = render_shares_conf(
            &[Share {
                name: "Copies".into(),
                path: "/srv/keel/copy".into(),
                protocol: "smb".into(),
                pool: "Nightly copy".into(),
                read_only: true,
            }],
            Path::new("/var/lib/keel/files"),
            Some("robby"),
        );
        assert!(conf.contains("guest ok = no"));
        assert!(conf.contains("valid users = robby"));
        assert!(conf.contains("read only = yes"));
        assert!(!conf.contains("guest ok = yes"));
    }
}
