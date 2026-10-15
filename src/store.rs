use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{NewNote, NewPool, NewShare, Note, Pool, Share};
use crate::remote::{AuthSession, ForgetHost, LoginRequest, SavedRemote, SavedRemotes};

#[derive(Default)]
pub struct Store {
    pub pools: Vec<Pool>,
    pub shares: Vec<Share>,
    pub notes: Vec<Note>,
    pub saved_remote: SavedRemotes,
    pub live: Option<AuthSession>,
}

impl Store {
    pub fn load() -> Self {
        Self {
            notes: load_notes(),
            saved_remote: load_remote(),
            pools: load_pools(),
            shares: load_shares(),
            ..Self::default()
        }
    }

    pub fn add_pool(&mut self, req: NewPool) -> Result<Pool, String> {
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err("name required".into());
        }
        if self.pools.iter().any(|p| p.name == name) {
            return Err("pool already exists".into());
        }
        let kind = match req.kind.as_str() {
            "foreign" | "local" => req.kind,
            _ => return Err("kind must be foreign or local".into()),
        };
        let source = req.source.trim().to_string();
        if kind == "foreign" && source.is_empty() {
            return Err(
                "for a pool on another computer, paste the folder address that machine shows you"
                    .into(),
            );
        }
        let pool = Pool {
            name,
            kind,
            source,
            size: 0,
            used: 0,
        };
        self.pools.push(pool.clone());
        let _ = save_pools(&self.pools);
        Ok(pool)
    }

    pub fn add_share(&mut self, req: NewShare) -> Result<Share, String> {
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err("name required".into());
        }
        if !crate::smb::valid_share_name(&name) {
            return Err("use letters, numbers, dash, or underscore for the share name".into());
        }
        if self.shares.iter().any(|item| item.name == name) {
            return Err("share already exists".into());
        }
        if self.pools.is_empty() {
            return Err("create a pool first".into());
        }
        if !self.pools.iter().any(|p| p.name == req.pool) {
            return Err("unknown pool".into());
        }
        let share = Share {
            name,
            pool: req.pool,
            path: req.path.trim().to_string(),
            protocol: "smb".into(),
            read_only: false,
        };
        self.shares.push(share.clone());
        save_shares(&self.shares)?;
        Ok(share)
    }

    pub fn ensure_default_share(&mut self, share: Share) {
        if self.shares.iter().any(|item| item.name == share.name) {
            return;
        }
        self.shares.push(share);
        let _ = save_shares(&self.shares);
    }

    pub fn add_note(&mut self, req: NewNote) -> Result<Note, String> {
        let note = Note {
            id: self.next_note_id(),
            title: req.title.trim().to_string(),
            body: req.body,
            updated_ms: now_ms(),
        };
        self.notes.insert(0, note.clone());
        self.save_notes()?;
        Ok(note)
    }

    pub fn update_note(&mut self, id: &str, req: NewNote) -> Result<Note, String> {
        let note = self
            .notes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| "unknown note".to_string())?;
        note.title = req.title.trim().to_string();
        note.body = req.body;
        note.updated_ms = now_ms();
        let out = note.clone();
        self.notes.retain(|n| n.id != id);
        self.notes.insert(0, out.clone());
        self.save_notes()?;
        Ok(out)
    }

    pub fn delete_note(&mut self, id: &str) -> Result<(), String> {
        let before = self.notes.len();
        self.notes.retain(|n| n.id != id);
        if self.notes.len() == before {
            return Err("unknown note".into());
        }
        self.save_notes()
    }

    fn next_note_id(&self) -> String {
        let max = self
            .notes
            .iter()
            .filter_map(|n| n.id.trim_start_matches('n').parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("n{}", max + 1)
    }

    fn save_notes(&self) -> Result<(), String> {
        save_notes(&self.notes)
    }

    pub fn remember_host(&mut self, host: &str, username: &str, realm: &str) {
        let _ = self.remember(host, username, realm, None);
    }

    /// Keep this computer and what answered last time. The password is not stored.
    pub fn remember_look(
        &mut self,
        host: &str,
        why: &str,
        hits: &[crate::discover::LookHit],
    ) -> Result<(), String> {
        self.remember(host, "", "", Some((why, hits)))
    }

    fn remember(
        &mut self,
        host: &str,
        username: &str,
        realm: &str,
        seen: Option<(&str, &[crate::discover::LookHit])>,
    ) -> Result<(), String> {
        let host = host.trim();
        if host.is_empty() {
            return Err("type a computer name or number".into());
        }
        let prev = self
            .saved_remote
            .hosts
            .iter()
            .find(|item| item.host.eq_ignore_ascii_case(host))
            .cloned();
        self.saved_remote
            .hosts
            .retain(|item| !item.host.eq_ignore_ascii_case(host));
        let username = if username.trim().is_empty() {
            prev.as_ref()
                .map(|item| item.username.clone())
                .unwrap_or_default()
        } else {
            username.trim().to_string()
        };
        let realm = if realm.trim().is_empty() {
            prev.as_ref()
                .map(|item| item.realm.clone())
                .filter(|item| !item.is_empty())
                .unwrap_or_else(|| "pam".into())
        } else {
            realm.trim().to_string()
        };
        let (why, hits) = if let Some((why, hits)) = seen {
            (why.to_string(), hits.to_vec())
        } else if let Some(prev) = prev {
            (prev.why, prev.hits)
        } else {
            (String::new(), Vec::new())
        };
        self.saved_remote.hosts.insert(
            0,
            SavedRemote {
                host: host.to_string(),
                username,
                realm,
                why,
                hits,
            },
        );
        save_remote(&self.saved_remote)
    }

    pub fn remember_remote(&mut self, req: &LoginRequest) {
        let host = crate::discover::normalize_host(&req.host)
            .unwrap_or_else(|_| req.host.trim().to_string());
        self.remember_host(&host, req.username.trim(), req.realm.trim());
    }

    pub fn forget_host(&mut self, req: &ForgetHost) -> Result<(), String> {
        let host = req.host.trim();
        if host.is_empty() {
            return Err("nothing to forget".into());
        }
        let before = self.saved_remote.hosts.len();
        self.saved_remote
            .hosts
            .retain(|h| !h.host.eq_ignore_ascii_case(host));
        if self.saved_remote.hosts.len() == before {
            return Err("that address was not saved".into());
        }
        save_remote(&self.saved_remote)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn notes_path() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("notes.json")
}

fn load_notes() -> Vec<Note> {
    let path = notes_path();
    fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_notes(notes: &[Note]) -> Result<(), String> {
    let path = notes_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(notes).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

fn remote_path() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("remote.json")
}

fn load_remote() -> SavedRemotes {
    let bytes = match fs::read(remote_path()) {
        Ok(b) => b,
        Err(_) => return SavedRemotes::default(),
    };
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return SavedRemotes::default(),
    };
    if value.get("hosts").is_some() {
        return serde_json::from_value(value).unwrap_or_default();
    }
    if let Ok(one) = serde_json::from_value::<SavedRemote>(value) {
        if !one.host.is_empty() {
            return SavedRemotes { hosts: vec![one] };
        }
    }
    SavedRemotes::default()
}

fn save_remote(saved: &SavedRemotes) -> Result<(), String> {
    let path = remote_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

fn pools_path() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("pools.json")
}

fn load_pools() -> Vec<Pool> {
    fs::read(pools_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_pools(pools: &[Pool]) -> Result<(), String> {
    let path = pools_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(pools).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

fn shares_path() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("shares.json")
}

fn load_shares() -> Vec<Share> {
    fs::read(shares_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_shares(shares: &[Share]) -> Result<(), String> {
    let path = shares_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(shares).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}
