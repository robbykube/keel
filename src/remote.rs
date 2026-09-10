use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, COOKIE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::discover::normalize_host;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub host: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub realm: String,
    #[serde(default)]
    pub insecure: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SavedRemote {
    pub host: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub realm: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SavedRemotes {
    #[serde(default)]
    pub hosts: Vec<SavedRemote>,
}

#[derive(Deserialize)]
pub struct ForgetHost {
    pub host: String,
}

#[derive(Clone)]
pub struct AuthSession {
    pub product: String,
    pub host: String,
    pub base: String,
    pub who: String,
    pub insecure: bool,
    pub ticket: String,
    pub csrf: String,
}

#[derive(Serialize, Clone)]
pub struct Place {
    pub title: String,
    pub kind: String,
    pub address: String,
    pub why: String,
    #[serde(default)]
    pub browse: String,
    #[serde(default)]
    pub open_url: String,
}

#[derive(Serialize)]
pub struct LoginReport {
    pub host: String,
    pub product: String,
    pub who: String,
    pub places: Vec<Place>,
    pub why: String,
}

fn client(insecure: bool) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(insecure)
        .cookie_store(true)
        .build()
        .map_err(|_| "could not start a web client".to_string())
}

fn bases(host: &str) -> Vec<String> {
    vec![
        format!("https://{host}"),
        format!("https://{host}:8006"),
        format!("http://{host}"),
    ]
}

fn pve_user(username: &str, realm: &str) -> String {
    let u = username.trim();
    if u.contains('@') {
        u.to_string()
    } else {
        let realm = if realm.trim().is_empty() {
            "pam"
        } else {
            realm.trim()
        };
        format!("{u}@{realm}")
    }
}

pub async fn https_hint(host: &str) -> Option<String> {
    let client = client(false).ok()?;
    for base in bases(host) {
        let url = format!("{base}/api2/json/version");
        if let Ok(res) = client.get(&url).send().await {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            if status.as_u16() == 401
                || body.contains("\"release\"")
                || body.contains("pve")
                || body.contains("proxmox")
            {
                return Some("This looks like Proxmox behind a website door. Port 8006 can stay quiet when a front door sits in front. Sign in below. The password stays on this PC in keel — do not paste it into chat.".into());
            }
        }
        let tn = format!("{base}/api/v2.0/system/info");
        if let Ok(res) = client.get(&tn).send().await {
            if res.status().as_u16() == 401 || res.status().as_u16() == 403 {
                return Some("This looks like TrueNAS. Sign in below to list folders. The password stays on this PC in keel — do not paste it into chat.".into());
            }
        }
    }
    None
}

pub fn http_client(insecure: bool) -> Result<reqwest::Client, String> {
    client(insecure)
}

pub fn pve_headers(session: &AuthSession) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    let cookie = format!("PVEAuthCookie={}", session.ticket);
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| "bad session cookie".to_string())?,
    );
    if let Ok(v) = HeaderValue::from_str(&session.csrf) {
        headers.insert("CSRFPreventionToken", v);
    }
    Ok(headers)
}

pub async fn login(req: &LoginRequest) -> Result<(LoginReport, Option<AuthSession>), String> {
    let host = normalize_host(&req.host)?;
    let username = req.username.trim();
    if username.is_empty() || req.password.is_empty() {
        return Err("name and password are required".into());
    }
    let client = client(req.insecure)?;
    let mut last = "sign-in did not match Proxmox or TrueNAS at that address".to_string();
    for base in bases(&host) {
        match pve_login(&client, &host, &base, username, &req.password, &req.realm, req.insecure).await {
            Ok((report, session)) => return Ok((report, Some(session))),
            Err(e) => last = e,
        }
        match tn_login(&client, &host, &base, username, &req.password).await {
            Ok(report) => return Ok((report, None)),
            Err(e) => {
                if last.starts_with("sign-in did not") {
                    last = e;
                }
            }
        }
    }
    Err(last)
}

#[derive(Deserialize)]
struct PveTicketWrap {
    data: PveTicket,
}

#[derive(Deserialize)]
struct PveTicket {
    ticket: String,
    #[serde(rename = "CSRFPreventionToken")]
    csrf: String,
    username: String,
}

async fn pve_login(
    client: &reqwest::Client,
    host: &str,
    base: &str,
    username: &str,
    password: &str,
    realm: &str,
    insecure: bool,
) -> Result<(LoginReport, AuthSession), String> {
    let user = pve_user(username, realm);
    let url = format!("{base}/api2/json/access/ticket");
    let res = client
        .post(&url)
        .form(&[("username", user.as_str()), ("password", password)])
        .send()
        .await
        .map_err(|_| "could not reach that website".to_string())?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if status.as_u16() == 401 || body.contains("invalid login") {
        return Err("sign-in refused. Check the name and password.".into());
    }
    if !status.is_success() {
        return Err("that website is not answering as Proxmox".into());
    }
    let wrap: PveTicketWrap =
        serde_json::from_str(&body).map_err(|_| "that website is not answering as Proxmox".to_string())?;

    let mut headers = HeaderMap::new();
    let cookie = format!("PVEAuthCookie={}", wrap.data.ticket);
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| "bad session cookie".to_string())?,
    );
    if let Ok(v) = HeaderValue::from_str(&wrap.data.csrf) {
        headers.insert("CSRFPreventionToken", v);
    }

    let storage = get_json(client, &format!("{base}/api2/json/storage"), &headers).await?;
    let mut nodes: Vec<String> = Vec::new();
    if let Ok(json) = get_json(client, &format!("{base}/api2/json/nodes"), &headers).await {
        if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
            for n in arr {
                if let Some(name) = n.get("node").and_then(|v| v.as_str()) {
                    nodes.push(name.to_string());
                }
            }
        }
    }
    let node = nodes.first().cloned().unwrap_or_else(|| host.to_string());
    let mut places = places_from_pve_storage(host, &node, &storage);
    for name in &nodes {
        places.extend(crate::files::guest_file_places(client, &headers, base, name).await);
        places.push(Place {
            title: format!("Node {name}"),
            kind: "proxmox-node".into(),
            address: String::new(),
            why: "This is the Proxmox machine itself, not a file folder. Leave it running. keel should not reimage it.".into(),
            browse: String::new(),
            open_url: String::new(),
        });
    }

    let session = AuthSession {
        product: "proxmox".into(),
        host: host.into(),
        base: base.into(),
        who: wrap.data.username.clone(),
        insecure,
        ticket: wrap.data.ticket,
        csrf: wrap.data.csrf,
    };

    Ok((
        LoginReport {
            host: host.into(),
            product: "proxmox".into(),
            who: session.who.clone(),
            places,
            why: "Signed in to Proxmox. Family files live in the files app (OpenCloud), not on local/vm-data. Use Check files to list. Shared folders still have a Use button. Do not point a pool at local.".into(),
        },
        session,
    ))
}

pub(crate) async fn get_json(client: &reqwest::Client, url: &str, headers: &HeaderMap) -> Result<Value, String> {
    let res = client
        .get(url)
        .headers(headers.clone())
        .send()
        .await
        .map_err(|_| "signed in, but listing storage failed".to_string())?;
    if !res.status().is_success() {
        return Err("signed in, but listing storage was refused".into());
    }
    res.json().await.map_err(|_| "signed in, but the storage list was not readable".to_string())
}

fn places_from_pve_storage(host: &str, node: &str, v: &Value) -> Vec<Place> {
    let Some(arr) = v.get("data").and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let name = item.get("storage")?.as_str()?.to_string();
            let kind = item
                .get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let path = item.get("path").and_then(|x| x.as_str()).unwrap_or("");
            let browse = format!("storage:{node}:{name}");
            match kind.as_str() {
                "cifs" | "smb" => {
                    let server = item.get("server").and_then(|x| x.as_str()).unwrap_or(host);
                    let share = item
                        .get("share")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| name.clone());
                    let address = format!(r"\\{server}\{share}");
                    Some(Place {
                        title: name,
                        kind: "windows-folder".into(),
                        address: address.clone(),
                        why: "A Windows-style folder already shared. Use it as a pool, or List files to check what is in it.".into(),
                        browse: format!("smb:{address}"),
                        open_url: String::new(),
                    })
                }
                "nfs" => {
                    let server = item.get("server").and_then(|x| x.as_str()).unwrap_or(host);
                    let export = item.get("export").and_then(|x| x.as_str()).unwrap_or(path);
                    Some(Place {
                        title: name,
                        kind: "nfs".into(),
                        address: format!("{server}:{export}"),
                        why: "A Unix-style folder share. Fine as a pool on another computer. List files shows what Proxmox sees on it.".into(),
                        browse,
                        open_url: String::new(),
                    })
                }
                "dir" if name == "local" => Some(Place {
                    title: name,
                    kind: "proxmox-disk".into(),
                    address: String::new(),
                    why: "Proxmox’s own disk (ISO/templates). Often already full. Not a family folder. You can still list what is on it.".into(),
                    browse,
                    open_url: String::new(),
                }),
                _ => Some(Place {
                    title: name,
                    kind: "proxmox-disk".into(),
                    address: String::new(),
                    why: format!("Proxmox storage ({kind}). Disks for VMs and LXCs, not a door you open on a laptop. List files shows the images. Family files are in the files app."),
                    browse,
                    open_url: String::new(),
                }),
            }
        })
        .collect()
}

async fn tn_login(
    client: &reqwest::Client,
    host: &str,
    base: &str,
    username: &str,
    password: &str,
) -> Result<LoginReport, String> {
    let url = format!("{base}/api/v2.0/auth/login");
    let res = client
        .post(&url)
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await
        .map_err(|_| "could not reach that website".to_string())?;
    let status = res.status();
    if status.as_u16() == 401 {
        return Err("sign-in refused. Check the name and password.".into());
    }
    if !status.is_success() {
        return Err("that website is not answering as TrueNAS".into());
    }

    let smb = client
        .get(format!("{base}/api/v2.0/sharing/smb"))
        .send()
        .await
        .map_err(|_| "signed in, but listing folders failed".to_string())?;
    let mut places = Vec::new();
    if smb.status().is_success() {
        if let Ok(list) = smb.json::<Value>().await {
            if let Some(arr) = list.as_array() {
                for item in arr {
                    let name = item
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("share")
                        .to_string();
                    let path = item.get("path").and_then(|x| x.as_str()).unwrap_or("");
                    places.push(Place {
                        title: name.clone(),
                        kind: "windows-folder".into(),
                        address: format!(r"\\{host}\{name}"),
                        why: format!("TrueNAS folder. On that machine it lives at {path}."),
                        browse: format!(r"smb:\\{host}\{name}"),
                        open_url: String::new(),
                    });
                }
            }
        }
    }
    let pools = client.get(format!("{base}/api/v2.0/pool")).send().await;
    if let Ok(res) = pools {
        if res.status().is_success() {
            if let Ok(list) = res.json::<Value>().await {
                if let Some(arr) = list.as_array() {
                    for item in arr {
                        let name = item.get("name").and_then(|x| x.as_str()).unwrap_or("");
                        if !name.is_empty() {
                            places.push(Place {
                                title: name.into(),
                                kind: "truenas-pool".into(),
                                address: format!("/mnt/{name}"),
                                why: "A TrueNAS bucket. Prefer a named Windows folder above if you see one — that is what a PC opens.".into(),
                                browse: String::new(),
                                open_url: String::new(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(LoginReport {
        host: host.into(),
        product: "truenas".into(),
        who: username.into(),
        places,
        why: "Signed in to TrueNAS. Named Windows folders are the addresses keel can use as a pool on another computer.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::pve_user;

    #[test]
    fn realm_appended_once() {
        assert_eq!(pve_user("root", "pam"), "root@pam");
        assert_eq!(pve_user("root@pam", "pam"), "root@pam");
    }
}
