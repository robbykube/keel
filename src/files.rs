use std::time::Duration;

use reqwest::header::{HeaderMap, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use serde_json::Value;

use crate::discover::port_open;
use crate::remote::{pve_headers, AuthSession, Place};

#[derive(Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub path: String,
    pub why: String,
    pub open_url: String,
    pub browse: String,
    pub address: String,
    #[serde(default)]
    pub proposal: String,
    #[serde(default)]
    pub their_name: String,
}

#[derive(Serialize)]
pub struct FilesReport {
    pub product: String,
    pub who: String,
    pub why: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Serialize)]
pub struct SessionStatus {
    pub signed_in: bool,
    pub product: String,
    pub who: String,
    pub host: String,
}

#[derive(Deserialize)]
pub struct BrowseRequest {
    pub via: String,
    pub target: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub insecure: bool,
}

#[derive(Serialize)]
pub struct BrowseReport {
    pub via: String,
    pub target: String,
    pub why: String,
    pub entries: Vec<FileEntry>,
}

const FILE_PORTS: &[(u16, &str)] = &[
    (445, "windows-folders"),
    (443, "https"),
    (80, "http"),
    (9200, "opencloud"),
    (8080, "http-alt"),
    (8443, "https-alt"),
];

/// Plain heading from what the other computer already named.
/// Shared-folder types (smb/nfs) are Use this. `local` is installer files.
/// Anything else is disks for other programs. We do not learn `vm-data` as a
/// special word — Proxmox sends that name; we only hide it from the heading.
pub fn describe_storage(name: &str, kind: &str) -> (String, String, String) {
    let n = name.to_ascii_lowercase();
    match kind {
        "cifs" | "smb" | "nfs" => (
            "Shared folder".into(),
            "use".into(),
            format!("This computer already shares a folder a PC can open. Use this. Name on that machine: {name}."),
        ),
        _ if n == "local" => (
            "Installer files on that computer".into(),
            "leave".into(),
            format!("Installers and templates for that system. Not family photos. You can list them to check. Name on that machine: {name}."),
        ),
        _ => (
            "Disks for other programs".into(),
            "leave".into(),
            format!("These disks belong to other programs on that computer. A laptop cannot open them as a folder. Family files are in the files app. Name on that machine: {name}."),
        ),
    }
}

/// Guess the files app from the name that computer already uses.
/// Guest 104 is this lab’s OpenCloud only — not a rule for other installs.
pub fn files_app_title(name: &str, vmid: u64) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("immich") {
        "Photos".into()
    } else if n.contains("papra") {
        "Documents".into()
    } else if n.contains("samba") {
        "Shared folders".into()
    } else if vmid == 104
        || n.contains("opencloud")
        || n.contains("nextcloud")
        || n.contains("owncloud")
        || n.contains("files")
    {
        "Family files".into()
    } else {
        "Files app".into()
    }
}

pub fn sort_key(proposal: &str) -> u8 {
    match proposal {
        "use" => 0,
        "list" => 1,
        _ => 2,
    }
}

pub fn session_status(live: Option<&AuthSession>) -> SessionStatus {
    match live {
        Some(s) => SessionStatus {
            signed_in: true,
            product: s.product.clone(),
            who: s.who.clone(),
            host: s.host.clone(),
        },
        None => SessionStatus {
            signed_in: false,
            product: String::new(),
            who: String::new(),
            host: String::new(),
        },
    }
}

pub async fn guest_file_places(
    client: &reqwest::Client,
    headers: &HeaderMap,
    base: &str,
    node: &str,
) -> Vec<Place> {
    let url = format!("{base}/api2/json/nodes/{node}/lxc");
    let Ok(json) = crate::remote::get_json(client, &url, headers).await else {
        return Vec::new();
    };
    let Some(arr) = json.get("data").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in arr {
        let vmid = item.get("vmid").and_then(|v| v.as_u64()).unwrap_or(0);
        if vmid == 0 {
            continue;
        }
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if !looks_like_files_app(&name, vmid) {
            continue;
        }
        let ips = guest_ips(client, headers, base, node, vmid).await;
        let mut reached = Vec::new();
        let mut open_url = String::new();
        let mut browse = String::new();
        let mut address = String::new();
        for ip in &ips {
            for (port, _label) in FILE_PORTS {
                if port_open(ip, *port).await {
                    if *port == 445 && address.is_empty() {
                        address = format!(r"\\{ip}\");
                        browse = format!("smb:{address}");
                        reached.push(format!("shared folder at {ip}"));
                    }
                    if open_url.is_empty() && matches!(*port, 443 | 80 | 9200 | 8080 | 8443) {
                        let scheme = if *port == 80 || *port == 8080 {
                            "http"
                        } else {
                            "https"
                        };
                        let url = if *port == 443 || *port == 80 {
                            format!("{scheme}://{ip}")
                        } else {
                            format!("{scheme}://{ip}:{port}")
                        };
                        open_url = url.clone();
                        if browse.is_empty() {
                            browse = format!("webdav:{url}");
                        }
                        reached.push(format!("website at {ip}"));
                    }
                }
            }
        }
        let title = files_app_title(&name, vmid);
        let why = if status != "running" {
            format!("{title}: this app is installed but not running. Start it on that computer if you need to see files. That computer calls it {name}.")
        } else if reached.is_empty() {
            format!(
                "{title}: keel could not reach this app from here (it is called {name}). Paste the files website you already open, below."
            )
        } else {
            format!(
                "{title} — this app is called {name}. Not the disks that belong to other programs. Open it in a browser, or list files here with the files-app password, not the Proxmox one. {}.",
                reached.join("; ")
            )
        };
        out.push(Place {
            title,
            kind: "files-app".into(),
            address,
            why,
            browse,
            open_url,
            proposal: "use".into(),
            their_name: name,
        });
    }
    out
}

async fn guest_ips(
    client: &reqwest::Client,
    headers: &HeaderMap,
    base: &str,
    node: &str,
    vmid: u64,
) -> Vec<String> {
    let mut ips = Vec::new();
    let cfg_url = format!("{base}/api2/json/nodes/{node}/lxc/{vmid}/config");
    if let Ok(cfg) = crate::remote::get_json(client, &cfg_url, headers).await {
        if let Some(data) = cfg.get("data").and_then(|v| v.as_object()) {
            for (k, v) in data {
                if k.starts_with("net") {
                    if let Some(s) = v.as_str() {
                        ips.extend(ips_from_net(s));
                    }
                }
            }
        }
    }
    let if_url = format!("{base}/api2/json/nodes/{node}/lxc/{vmid}/interfaces");
    if let Ok(json) = crate::remote::get_json(client, &if_url, headers).await {
        if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                for key in ["inet", "address", "ip"] {
                    if let Some(s) = item.get(key).and_then(|v| v.as_str()) {
                        ips.extend(ips_from_cidr(s));
                    }
                }
            }
        }
    }
    ips.sort();
    ips.dedup();
    ips.retain(|ip| ip != "127.0.0.1" && ip != "::1");
    ips
}

fn looks_like_files_app(name: &str, vmid: u64) -> bool {
    let n = name.to_ascii_lowercase();
    vmid == 104
        || n.contains("opencloud")
        || n.contains("nextcloud")
        || n.contains("owncloud")
        || n.contains("immich")
        || n.contains("papra")
        || n.contains("samba")
        || n.contains("files")
}

fn ips_from_net(s: &str) -> Vec<String> {
    s.split(',')
        .filter_map(|part| {
            let part = part.trim();
            part.strip_prefix("ip=").map(ips_from_cidr)
        })
        .flatten()
        .collect()
}

fn ips_from_cidr(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("dhcp") || s.eq_ignore_ascii_case("manual") {
        return Vec::new();
    }
    let ip = s.split('/').next().unwrap_or(s).trim();
    if ip.chars().all(|c| c.is_ascii_digit() || c == '.') {
        vec![ip.to_string()]
    } else {
        Vec::new()
    }
}

pub async fn inventory(session: &AuthSession) -> Result<FilesReport, String> {
    if session.product != "proxmox" {
        return Err(
            "Check files after a Proxmox sign-in. TrueNAS folders already have a Use button."
                .into(),
        );
    }
    let client = crate::remote::http_client(session.insecure)?;
    let headers = pve_headers(session)?;
    let nodes = first_nodes(&client, &headers, &session.base).await;
    let mut entries = Vec::new();
    for node in &nodes {
        for place in guest_file_places(&client, &headers, &session.base, node).await {
            entries.push(FileEntry {
                name: place.title.clone(),
                kind: place.kind.clone(),
                size: 0,
                path: place.browse.clone(),
                why: place.why.clone(),
                open_url: place.open_url.clone(),
                browse: place.browse.clone(),
                address: place.address.clone(),
                proposal: place.proposal.clone(),
                their_name: place.their_name.clone(),
            });
        }
        let stor_url = format!("{}/api2/json/nodes/{node}/storage", session.base);
        if let Ok(json) = crate::remote::get_json(&client, &stor_url, &headers).await {
            if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
                for item in arr {
                    let name = item
                        .get("storage")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let used = item.get("used").and_then(|v| v.as_u64()).unwrap_or(0);
                    let (title, proposal, why) = describe_storage(&name, kind);
                    entries.push(FileEntry {
                        name: title,
                        kind: if proposal == "use" {
                            "shared-folder".into()
                        } else {
                            "leave".into()
                        },
                        size: used,
                        path: format!("storage:{node}:{name}"),
                        why,
                        open_url: String::new(),
                        browse: format!("storage:{node}:{name}"),
                        address: String::new(),
                        proposal,
                        their_name: name,
                    });
                }
            }
        }
    }
    entries.sort_by_key(|e| sort_key(&e.proposal));
    Ok(FilesReport {
        product: session.product.clone(),
        who: session.who.clone(),
        why: "Family files are in the files app. Do not use disks that belong to other programs on that computer. Shared folders have Use this. The sign-in stays in memory until keel restarts. The password is not saved to disk.".into(),
        entries,
    })
}

async fn first_nodes(client: &reqwest::Client, headers: &HeaderMap, base: &str) -> Vec<String> {
    let url = format!("{base}/api2/json/nodes");
    let Ok(json) = crate::remote::get_json(client, &url, headers).await else {
        return Vec::new();
    };
    json.get("data")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| {
                    n.get("node")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn browse(
    session: Option<&AuthSession>,
    req: &BrowseRequest,
) -> Result<BrowseReport, String> {
    match req.via.as_str() {
        "storage" => {
            let session =
                session.ok_or_else(|| "sign in to Proxmox on this page first".to_string())?;
            browse_storage(session, &req.target).await
        }
        "webdav" => browse_webdav(req).await,
        "smb" => browse_smb(req),
        _ => Err("unknown way to list files".into()),
    }
}

async fn browse_storage(session: &AuthSession, target: &str) -> Result<BrowseReport, String> {
    let rest = target.strip_prefix("storage:").unwrap_or(target);
    let (node, storage) = rest
        .split_once(':')
        .ok_or_else(|| "that storage name was not usable".to_string())?;
    let client = crate::remote::http_client(session.insecure)?;
    let headers = pve_headers(session)?;
    let url = format!(
        "{}/api2/json/nodes/{node}/storage/{storage}/content",
        session.base
    );
    let json = crate::remote::get_json(&client, &url, &headers).await?;
    let mut entries = Vec::new();
    if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
        for item in arr.iter().take(200) {
            let volid = item
                .get("volid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("file")
                .to_string();
            let size = item.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            let vol_name = volid.rsplit('/').next().unwrap_or(&volid).to_string();
            let (title, proposal, why) = match content.as_str() {
                "iso" => (
                    vol_name.clone(),
                    "list".to_string(),
                    format!("An installer file on that computer. Not a family photo. Name: {vol_name}."),
                ),
                "vztmpl" => (
                    "Program template".into(),
                    "leave".into(),
                    format!("A template for another program. Not a shared folder. Name: {vol_name}."),
                ),
                "backup" => (
                    vol_name.clone(),
                    "list".to_string(),
                    format!("A backup file. Name: {vol_name}."),
                ),
                "images" | "rootdir" => (
                    "Disk for another program".into(),
                    "leave".into(),
                    format!("A disk used by another program on that computer. A laptop cannot open this as a folder. Name: {vol_name}."),
                ),
                "snippets" => (
                    vol_name.clone(),
                    "list".to_string(),
                    format!("A small config file. Name: {vol_name}."),
                ),
                _ => (
                    vol_name.clone(),
                    "list".to_string(),
                    format!("A file on that computer. Name: {vol_name}."),
                ),
            };
            entries.push(FileEntry {
                name: title,
                kind: proposal.clone(),
                size,
                path: volid,
                why,
                open_url: String::new(),
                browse: String::new(),
                address: String::new(),
                proposal,
                their_name: vol_name,
            });
        }
    }
    let why = if entries.is_empty() {
        "Nothing listed here (empty, or this kind of disk does not show files).".into()
    } else {
        format!("{} items. These are not family photos. They are what that computer keeps for its own programs.", entries.len())
    };
    Ok(BrowseReport {
        via: "storage".into(),
        target: target.into(),
        why,
        entries,
    })
}

async fn browse_webdav(req: &BrowseRequest) -> Result<BrowseReport, String> {
    let base = req
        .target
        .strip_prefix("webdav:")
        .unwrap_or(req.target.trim())
        .trim_end_matches('/')
        .to_string();
    if base.is_empty() {
        return Err("no files-app address".into());
    }
    if req.username.trim().is_empty() || req.password.is_empty() {
        return Err(
            "sign in to the files app (OpenCloud) — that is not the Proxmox password".into(),
        );
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .danger_accept_invalid_certs(req.insecure)
        .build()
        .map_err(|_| "could not start a web client".to_string())?;
    let user = req.username.trim();
    let path = if req.path.trim().is_empty() {
        String::new()
    } else {
        req.path.trim().to_string()
    };
    let candidates = dav_urls(&base, user, &path);
    let body = r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:displayname/><d:getcontentlength/><d:resourcetype/></d:prop></d:propfind>"#;
    let mut last = "the files app did not answer WebDAV".to_string();
    for url in candidates {
        let res = client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .basic_auth(user, Some(&req.password))
            .header(CONTENT_TYPE, "application/xml")
            .header("Depth", "1")
            .body(body)
            .send()
            .await;
        match res {
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                if status.as_u16() == 401 || status.as_u16() == 403 {
                    last = "files app refused that name or password".into();
                    continue;
                }
                if !status.is_success() && status.as_u16() != 207 {
                    last = format!("files app answered {status}");
                    continue;
                }
                let mut entries = parse_dav(&text, &url);
                entries.retain(|e| e.path.trim_end_matches('/') != url.trim_end_matches('/'));
                return Ok(BrowseReport {
                    via: "webdav".into(),
                    target: req.target.clone(),
                    why: format!(
                        "Listed {} from the files app. This is read-only in keel. Password stayed in memory for this request only.",
                        entries.len()
                    ),
                    entries,
                });
            }
            Err(_) => last = "could not reach the files app at that address".into(),
        }
    }
    Err(last)
}

fn dav_urls(base: &str, user: &str, extra: &str) -> Vec<String> {
    let extra = extra.trim_start_matches('/');
    let suffix = if extra.is_empty() {
        String::new()
    } else {
        format!("{extra}/")
    };
    vec![
        format!("{base}/remote.php/dav/files/{user}/{suffix}"),
        format!("{base}/dav/files/{user}/{suffix}"),
        format!("{base}/remote.php/webdav/{suffix}"),
        format!("{base}/webdav/{suffix}"),
        format!("{base}/dav/{suffix}"),
    ]
}

fn parse_dav(xml: &str, parent: &str) -> Vec<FileEntry> {
    let lower = xml.to_ascii_lowercase();
    let mut entries = Vec::new();
    let mut from = 0;
    while let Some(start) = lower[from..]
        .find("<d:href>")
        .or_else(|| lower[from..].find("<href>"))
    {
        let abs = from + start;
        let tag_end = lower[abs..].find('>').map(|i| abs + i + 1).unwrap_or(abs);
        let Some(end_rel) = lower[tag_end..]
            .find("</d:href>")
            .or_else(|| lower[tag_end..].find("</href>"))
        else {
            break;
        };
        let href = xml[tag_end..tag_end + end_rel].trim();
        let href = html_unescape(href);
        from = tag_end + end_rel + 1;
        let chunk_end = lower[from..]
            .find("<d:href>")
            .or_else(|| lower[from..].find("<href>"))
            .map(|i| from + i)
            .unwrap_or(xml.len());
        let chunk = &lower[from..chunk_end];
        let folder = chunk.contains("<d:collection") || chunk.contains("<collection/");
        let size = extract_size(chunk);
        let name = href
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&href)
            .to_string();
        if name.is_empty() || href == parent {
            continue;
        }
        entries.push(FileEntry {
            name: urlencoding_decode(&name),
            kind: if folder {
                "folder".into()
            } else {
                "file".into()
            },
            size,
            path: href.clone(),
            why: if folder {
                "A folder in the files app.".into()
            } else {
                "A file in the files app. keel lists it; open the files app to edit.".into()
            },
            open_url: href,
            browse: if folder {
                format!("webdav-path:{name}")
            } else {
                String::new()
            },
            address: String::new(),
            proposal: if folder { "use".into() } else { "list".into() },
            their_name: urlencoding_decode(&name),
        });
        if entries.len() >= 200 {
            break;
        }
    }
    entries
}

fn extract_size(chunk: &str) -> u64 {
    let key = "getcontentlength>";
    if let Some(i) = chunk.find(key) {
        let rest = &chunk[i + key.len()..];
        if let Some(end) = rest.find('<') {
            return rest[..end].trim().parse().unwrap_or(0);
        }
    }
    0
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn browse_smb(req: &BrowseRequest) -> Result<BrowseReport, String> {
    let target = req
        .target
        .strip_prefix("smb:")
        .unwrap_or(&req.target)
        .replace('/', "\\");
    let target = target.trim().trim_end_matches('\\').to_string();
    if !target.starts_with("\\\\") {
        return Err("that is not a Windows folder address".into());
    }
    let parts: Vec<&str> = target
        .trim_start_matches('\\')
        .split('\\')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("that is not a Windows folder address".into());
    }
    #[cfg(not(windows))]
    {
        let _ = (req, parts);
        return Err("listing Windows folders from this OS is later".into());
    }
    #[cfg(windows)]
    {
        smb_list_windows(&target, req.username.trim(), &req.password)
    }
}

#[cfg(windows)]
fn smb_list_windows(unc: &str, username: &str, password: &str) -> Result<BrowseReport, String> {
    let parts: Vec<&str> = unc
        .trim_start_matches('\\')
        .split('\\')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() == 1 {
        return smb_view_windows(parts[0], username, password);
    }
    if username.is_empty() {
        return read_unc_dir(unc);
    }
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"
            $root = $env:KEEL_SMB_ROOT
            $user = $env:KEEL_SMB_USER
            $pass = ConvertTo-SecureString $env:KEEL_SMB_PASS -AsPlainText -Force
            $cred = New-Object System.Management.Automation.PSCredential($user, $pass)
            New-PSDrive -Name KeelSmb -PSProvider FileSystem -Root $root -Credential $cred -ErrorAction Stop | Out-Null
            Get-ChildItem -LiteralPath KeelSmb:\ | Select-Object Name, Length, PSIsContainer | ConvertTo-Json -Compress
            Remove-PSDrive KeelSmb -ErrorAction SilentlyContinue
            "#,
        ])
        .env("KEEL_SMB_ROOT", unc)
        .env("KEEL_SMB_USER", username)
        .env("KEEL_SMB_PASS", password)
        .output()
        .map_err(|_| "could not ask Windows to list that folder".to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(if err.trim().is_empty() {
            "Windows would not open that folder. Check the share name and files-app user.".into()
        } else {
            "Windows would not open that folder. Check the share name and files-app user.".into()
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(BrowseReport {
        via: "smb".into(),
        target: unc.into(),
        why: "Listed from a Windows folder share. Read-only in keel.".into(),
        entries: parse_ps_children(&stdout),
    })
}

#[cfg(windows)]
fn smb_view_windows(host: &str, username: &str, password: &str) -> Result<BrowseReport, String> {
    let _ = (username, password);
    let output = std::process::Command::new("net")
        .args(["view", &format!(r"\\{host}")])
        .output()
        .map_err(|_| "could not list shares on that computer".to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Share")
            || line.starts_with("---")
            || line.starts_with("The command")
        {
            continue;
        }
        let name = line.split_whitespace().next().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let address = format!(r"\\{host}\{name}");
        entries.push(FileEntry {
            name: name.into(),
            kind: "share".into(),
            size: 0,
            path: address.clone(),
            why: "A named Windows folder on that computer.".into(),
            open_url: String::new(),
            browse: format!("smb:{address}"),
            address,
            proposal: "use".into(),
            their_name: name.into(),
        });
    }
    if entries.is_empty() {
        return Err("no Windows folders published, or this PC is not allowed to see them".into());
    }
    Ok(BrowseReport {
        via: "smb".into(),
        target: format!(r"\\{host}"),
        why: "Shares on that computer. Pick one and List files with the folder user.".into(),
        entries,
    })
}

#[cfg(windows)]
fn read_unc_dir(unc: &str) -> Result<BrowseReport, String> {
    let rd = std::fs::read_dir(unc).map_err(|_| {
        "this PC cannot open that folder without a name and password. Sign in to the files share below.".to_string()
    })?;
    let mut entries = Vec::new();
    for ent in rd.flatten().take(200) {
        let name = ent.file_name().to_string_lossy().into_owned();
        let meta = ent.metadata().ok();
        let folder = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.map(|m| m.len()).unwrap_or(0);
        let path = format!("{unc}\\{name}");
        entries.push(FileEntry {
            name: name.clone(),
            kind: if folder {
                "folder".into()
            } else {
                "file".into()
            },
            size,
            path: path.clone(),
            why: "On that Windows folder share.".into(),
            open_url: String::new(),
            browse: if folder {
                format!("smb:{path}")
            } else {
                String::new()
            },
            address: path,
            proposal: if folder { "use".into() } else { "list".into() },
            their_name: name,
        });
    }
    Ok(BrowseReport {
        via: "smb".into(),
        target: unc.into(),
        why: "Listed with this PC’s existing Windows sign-in.".into(),
        entries,
    })
}

#[cfg(windows)]
fn parse_ps_children(stdout: &str) -> Vec<FileEntry> {
    let text = stdout.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let arr = match value {
        Value::Array(a) => a,
        Value::Object(_) => vec![value],
        _ => return Vec::new(),
    };
    arr.iter()
        .filter_map(|item| {
            let name = item.get("Name")?.as_str()?.to_string();
            let folder = item
                .get("PSIsContainer")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let size = item.get("Length").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(FileEntry {
                name: name.clone(),
                kind: if folder {
                    "folder".into()
                } else {
                    "file".into()
                },
                size,
                path: name.clone(),
                why: "On that Windows folder share.".into(),
                open_url: String::new(),
                browse: String::new(),
                address: name.clone(),
                proposal: if folder { "use".into() } else { "list".into() },
                their_name: name,
            })
        })
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{describe_storage, ips_from_net, looks_like_files_app, parse_dav};

    #[test]
    fn storage_talks_plain() {
        let (title, proposal, why) = describe_storage("vm-data", "zfspool");
        assert_eq!(title, "Disks for other programs");
        assert_eq!(proposal, "leave");
        assert!(why.contains("vm-data"));
        let (title, proposal, _) = describe_storage("photos", "cifs");
        assert_eq!(title, "Shared folder");
        assert_eq!(proposal, "use");
        let (title, proposal, _) = describe_storage("local", "dir");
        assert_eq!(title, "Installer files on that computer");
        assert_eq!(proposal, "leave");
    }

    #[test]
    fn net0_static_ip() {
        let ips = ips_from_net("name=eth0,bridge=vmbr0,ip=192.168.1.40/24,type=veth");
        assert_eq!(ips, vec!["192.168.1.40"]);
    }

    #[test]
    fn opencloud_is_files() {
        assert!(looks_like_files_app("opencloud", 1));
        assert!(looks_like_files_app("mail", 104));
        assert!(!looks_like_files_app("vaultwarden", 103));
    }

    #[test]
    fn dav_hrefs() {
        let xml = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">
            <d:response><d:href>/dav/files/u/Photos/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response>
            <d:response><d:href>/dav/files/u/note.txt</d:href><d:propstat><d:prop><d:getcontentlength>12</d:getcontentlength><d:resourcetype/></d:prop></d:propstat></d:response>
        </d:multistatus>"#;
        let entries = parse_dav(xml, "https://x/dav/files/u/");
        assert!(entries
            .iter()
            .any(|e| e.name == "Photos" && e.kind == "folder"));
        assert!(entries.iter().any(|e| e.name == "note.txt" && e.size == 12));
    }
}
