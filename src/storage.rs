//! Disks in this computer: which job each one has, getting an empty disk
//! ready for that job, and who may open the folders.
//!
//! Keel's own service is sandboxed (`ProtectSystem=full`), so it cannot write
//! `/etc/fstab`, and a disk it mounted would only exist inside its own mount
//! namespace, out of Samba's sight. Work that changes the host runs through
//! `systemd-run` as a one-off unit outside that sandbox.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::model::{Disk, Issue, Share};
use crate::store::Store;

pub const MOUNT_ROOT: &str = "/srv/keel";

pub struct Role {
    pub id: &'static str,
    pub title: &'static str,
    pub what: &'static str,
    /// Folders made on this disk and opened as shared folders.
    pub folders: &'static [&'static str],
}

pub const ROLES: &[Role] = &[
    Role {
        id: "files",
        title: "Files",
        what: "Documents, Work and Media.",
        folders: &["Documents", "Work", "Media"],
    },
    Role {
        id: "copy",
        title: "Nightly copy",
        what: "A copy of Files, made once a day. Older versions are kept for 30 days and open as the Copies folder.",
        folders: &[],
    },
    Role {
        id: "extra",
        title: "Third copy",
        what: "Another copy of Documents and Work only, the most important folders.",
        folders: &[],
    },
    Role {
        id: "software",
        title: "Software and Inbox",
        what: "Installers, games, and unsorted files. Not copied; all of it can be downloaded again.",
        folders: &["Software", "Inbox"],
    },
];

/// The read-only folder that shows the nightly copy, so a person can take
/// yesterday's version back without asking anyone.
const COPIES_SHARE: &str = "Copies";

pub fn role(id: &str) -> Option<&'static Role> {
    ROLES.iter().find(|role| role.id == id)
}

pub fn mount_point(role: &str) -> PathBuf {
    Path::new(MOUNT_ROOT).join(role)
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct RoleRecord {
    pub model: String,
    pub size: u64,
    pub prepared_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CopyStatus {
    pub last_run_ms: u64,
    pub last_ok_ms: u64,
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct State {
    owner: Option<String>,
    roles: BTreeMap<String, RoleRecord>,
    copies: CopyStatus,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(load()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn state_path() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("storage.json")
}

fn load() -> State {
    std::fs::read(state_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(state: &State) {
    let path = state_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(body) = serde_json::to_vec_pretty(state) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, body).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn owner() -> Option<String> {
    state().owner.clone()
}

pub fn copy_status() -> CopyStatus {
    state().copies.clone()
}

pub fn set_copy_status(status: CopyStatus) {
    let mut state = state();
    state.copies = status;
    save(&state);
}

pub fn prepared_roles() -> Vec<String> {
    state().roles.keys().cloned().collect()
}

/// Whether the disk for `role` is mounted where Keel expects it. Read from
/// Keel's own mount table: host mounts propagate into the service.
pub fn mounted(role: &str) -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    let target = mount_point(role);
    let target = target.to_string_lossy();
    std::fs::read_to_string("/proc/self/mounts")
        .map(|mounts| {
            mounts
                .lines()
                .any(|line| line.split_whitespace().nth(1) == Some(target.as_ref()))
        })
        .unwrap_or(false)
}

/// The role a share's folder lives on, if it lives on one of Keel's disks.
pub fn share_role(path: &str) -> Option<String> {
    let rest = path.strip_prefix(MOUNT_ROOT)?.strip_prefix('/')?;
    let role = rest.split('/').next()?;
    (!role.is_empty()).then(|| role.to_string())
}

#[derive(Serialize)]
pub struct DiskView {
    #[serde(flatten)]
    pub disk: Disk,
    /// What is on this disk, in words.
    pub holds: String,
    pub role: Option<String>,
    pub can_use: bool,
}

#[derive(Serialize)]
pub struct RoleView {
    pub id: &'static str,
    pub title: &'static str,
    pub what: &'static str,
    pub disk: Option<String>,
    pub ready: bool,
}

#[derive(Serialize)]
pub struct StorageView {
    pub disks: Vec<DiskView>,
    pub roles: Vec<RoleView>,
    pub owner: Option<String>,
    pub copies: CopyStatus,
    pub copy_running: bool,
    pub can_copy: bool,
}

pub fn view(disks: Vec<Disk>) -> StorageView {
    let disks: Vec<DiskView> = disks
        .into_iter()
        .filter(|disk| disk.kind == "disk")
        .map(describe)
        .collect();
    let roles = ROLES
        .iter()
        .map(|role| RoleView {
            id: role.id,
            title: role.title,
            what: role.what,
            disk: disks
                .iter()
                .find(|d| d.role.as_deref() == Some(role.id))
                .map(|d| disk_words(&d.disk)),
            ready: mounted(role.id),
        })
        .collect();
    StorageView {
        disks,
        roles,
        owner: owner(),
        copies: copy_status(),
        copy_running: crate::copies::running(),
        can_copy: crate::copies::can_run(),
    }
}

fn disk_words(disk: &Disk) -> String {
    let model = if disk.model.is_empty() { "Disk" } else { disk.model.as_str() };
    format!("{model} ({})", disk.name)
}

fn describe(disk: Disk) -> DiskView {
    let role = disk
        .label
        .strip_prefix("keel-")
        .filter(|id| role(id).is_some())
        .map(str::to_string);
    let holds = if disk.system {
        "Keel itself".to_string()
    } else if let Some(id) = role.as_deref() {
        let title = self::role(id).map(|r| r.title).unwrap_or(id);
        if disk.mount.is_empty() {
            format!("{title} (not open yet)")
        } else {
            title.to_string()
        }
    } else if !disk.mount.is_empty() {
        "In use by this computer".to_string()
    } else if disk.empty {
        "Empty".to_string()
    } else {
        "Old data. Using it erases that.".to_string()
    };
    let can_use = !disk.system && disk.mount.is_empty() && role.is_none() && !disk.path.is_empty();
    DiskView {
        disk,
        holds,
        role,
        can_use,
    }
}

#[derive(Deserialize)]
pub struct PrepareRequest {
    pub path: String,
    pub role: String,
    pub confirm: String,
}

pub fn prepare(request: &PrepareRequest, disks: &[Disk]) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("Disks are set up on the Keel computer, not on this Windows copy.".into());
    }
    let job = role(&request.role).ok_or_else(|| "Choose what this disk is for.".to_string())?;
    let view = view(disks.to_vec());
    let target = view
        .disks
        .iter()
        .find(|d| d.disk.path == request.path)
        .ok_or_else(|| "That disk is no longer connected. Reload the page.".to_string())?;
    if !target.can_use {
        return Err(format!("{} cannot be used: {}.", target.disk.name, target.holds));
    }
    if request.confirm.trim() != target.disk.name {
        return Err(format!(
            "Type {} exactly to erase that disk.",
            target.disk.name
        ));
    }
    if let Some(other) = view.disks.iter().find(|d| d.role.as_deref() == Some(job.id)) {
        return Err(format!(
            "{} already holds {}. Each job uses one disk.",
            disk_words(&other.disk),
            job.title
        ));
    }
    host(
        PREPARE_SCRIPT,
        &[&target.disk.path, job.id],
        None,
    )?;
    {
        let mut state = state();
        state.roles.insert(
            job.id.to_string(),
            RoleRecord {
                model: target.disk.model.clone(),
                size: target.disk.size,
                prepared_ms: now_ms(),
            },
        );
        save(&state);
    }
    if let Some(name) = owner() {
        let dir = mount_point(job.id).to_string_lossy().to_string();
        let _ = host(CHOWN_SCRIPT, &[&name, &dir], None);
    }
    Ok(format!("{} is ready for {}.", disk_words(&target.disk), job.title))
}

/// A section beside Documents, Work, and Media. It lives on the Files disk,
/// so the nightly copy includes it, and a computer opens it by this name.
pub fn add_section(name: &str) -> Result<Share, String> {
    let name = name.trim();
    if !crate::folders::valid_entry_name(name) || !crate::smb::valid_share_name(name) {
        return Err("Use a name with letters, numbers, and spaces. Leave out symbols.".into());
    }
    if name.eq_ignore_ascii_case("files") || name.eq_ignore_ascii_case("copies") {
        return Err("That name is already used.".into());
    }
    if !cfg!(target_os = "linux") {
        return Err("New sections are added on the Keel computer.".into());
    }
    if !mounted("files") {
        return Err("Set up the Files disk first. Documents, Work, and Media live there.".into());
    }
    let dir = mount_point("files").join(name);
    if dir.exists() {
        return Err("That section is already there.".into());
    }
    std::fs::create_dir(&dir).map_err(|_| "Keel could not make that section. Try again.".to_string())?;
    give_to_owner(&dir);
    Ok(Share {
        name: name.to_string(),
        path: dir.to_string_lossy().into(),
        protocol: "smb".into(),
        pool: "Files".into(),
        read_only: false,
    })
}

/// Shares for every prepared disk. Existing shares with the same name are
/// left alone, so a renamed or removed folder does not come back by itself.
pub fn ensure_shares(store: &mut Store) {
    for id in prepared_roles() {
        let Some(job) = role(&id) else { continue };
        for folder in job.folders {
            store.ensure_default_share(Share {
                name: (*folder).to_string(),
                path: mount_point(job.id).join(folder).to_string_lossy().into(),
                protocol: "smb".into(),
                pool: job.title.into(),
                read_only: false,
            });
        }
        if job.id == "copy" {
            store.ensure_default_share(Share {
                name: COPIES_SHARE.into(),
                path: mount_point(job.id).to_string_lossy().into(),
                protocol: "smb".into(),
                pool: job.title.into(),
                read_only: true,
            });
        }
    }
}

#[derive(Deserialize)]
pub struct OwnerRequest {
    pub name: String,
    pub password: String,
}

const RESERVED_NAMES: &[&str] = &[
    "root", "keel", "nobody", "daemon", "bin", "sys", "admin", "guest", "samba", "sshd",
    "keel-kiosk", "user", "live",
];

pub fn valid_owner_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && name.len() >= 2
        && name.len() <= 31
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !RESERVED_NAMES.contains(&name)
}

/// Set (or change the password of) the one name that opens every folder.
/// `share_dirs` are the writable folders that should belong to that name.
pub fn set_owner(request: &OwnerRequest, share_dirs: &[String]) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("Folder sign-in is set on the Keel computer, not on this Windows copy.".into());
    }
    let name = request.name.trim().to_lowercase();
    if let Some(current) = owner() {
        if current != name {
            return Err(format!(
                "These folders already open with {current}. Change the password for {current}."
            ));
        }
    }
    if !valid_owner_name(&name) {
        return Err(
            "Use 2 to 31 small letters or numbers, starting with a letter. A dash is fine.".into(),
        );
    }
    let password = &request.password;
    if password.chars().count() < 8 {
        return Err("Use at least 8 characters for the password.".into());
    }
    if password.contains('\n') || password.contains('\r') {
        return Err("The password cannot contain a line break.".into());
    }
    let mut args: Vec<&str> = vec![&name];
    args.extend(share_dirs.iter().map(String::as_str));
    host(OWNER_SCRIPT, &args, Some(&format!("{password}\n")))?;
    {
        let mut state = state();
        state.owner = Some(name.clone());
        save(&state);
    }
    Ok(format!("The folders now open only with {name} and your password."))
}

pub fn issues() -> Vec<Issue> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    let mut items = Vec::new();
    let missing: Vec<String> = prepared_roles()
        .into_iter()
        .filter(|id| !mounted(id))
        .map(|id| role(&id).map(|r| r.title.to_string()).unwrap_or(id))
        .collect();
    if !missing.is_empty() {
        items.push(Issue {
            id: "disk-missing",
            severity: "red",
            title: "A Keel disk is not connected",
            why: "Its folders are hidden until it is back, so nothing is written to the wrong disk. Check the Yottamaster's power and cable. Keel opens the disk again by itself.",
            evidence: missing.join(", "),
            repair: None,
        });
    }
    let copies = copy_status();
    let day = 24 * 60 * 60 * 1000;
    if crate::copies::can_run() && copies.last_run_ms > 0 && !copies.ok {
        items.push(Issue {
            id: "copy-failed",
            severity: "yellow",
            title: "The last copy had problems",
            why: "Some files were not copied. The next copy tries again. Disks shows Copy now.",
            evidence: copies.message.clone(),
            repair: None,
        });
    } else if crate::copies::can_run()
        && copies.last_ok_ms > 0
        && now_ms().saturating_sub(copies.last_ok_ms) > 2 * day
    {
        items.push(Issue {
            id: "copy-late",
            severity: "yellow",
            title: "Copies have not finished for two days",
            why: "Keel copies once a day. Open Disks and press Copy now.",
            evidence: copies.message.clone(),
            repair: None,
        });
    }
    items
}

/// Run a script outside Keel's sandbox and wait for it. The script's last
/// line on stderr is the sentence shown when it fails.
fn host(script: &str, args: &[&str], input: Option<&str>) -> Result<(), String> {
    let mut command = if Path::new("/usr/bin/systemd-run").exists() || Path::new("/bin/systemd-run").exists() {
        let mut c = Command::new("systemd-run");
        c.args(["--quiet", "--wait", "--pipe", "--collect", "--", "/bin/bash", "-c", script, "keel-host"]);
        c
    } else {
        let mut c = Command::new("/bin/bash");
        c.args(["-c", script, "keel-host"]);
        c
    };
    command
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Keel could not start the disk helper: {error}"))?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("The disk helper stopped: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .last()
        .unwrap_or("The disk helper failed without saying why.")
        .to_string())
}

const PREPARE_SCRIPT: &str = r#"set -uo pipefail
dev="$1"; role="$2"; mp="/srv/keel/$role"
fail() { echo "$1" >&2; exit 1; }
[ -b "$dev" ] || fail "That disk is no longer connected."
if lsblk -nro MOUNTPOINT "$dev" | grep -q .; then fail "That disk is in use. Keel did not change it."; fi
for p in $(lsblk -lnpo NAME,TYPE "$dev" | awk '$2=="part"{print $1}'); do wipefs -aq "$p" >/dev/null 2>&1 || true; done
wipefs -aq "$dev" >/dev/null 2>&1 || fail "Keel could not clear that disk."
parted -s "$dev" mklabel gpt mkpart "keel-$role" ext4 1MiB 100% >/dev/null 2>&1 || fail "Keel could not make a new layout on that disk."
partprobe "$dev" >/dev/null 2>&1 || true
udevadm settle >/dev/null 2>&1 || true
part=""
for _ in $(seq 1 20); do
  part=$(lsblk -lnpo NAME,TYPE "$dev" | awk '$2=="part"{print $1; exit}')
  [ -n "$part" ] && [ -b "$part" ] && break
  sleep 0.5
done
[ -n "$part" ] && [ -b "$part" ] || fail "The new space on that disk did not appear. Unplug it, plug it back in, and try again."
wipefs -aq "$part" >/dev/null 2>&1 || true
mkfs.ext4 -F -q -m 0 -L "keel-$role" "$part" >/dev/null 2>&1 || fail "Keel could not format that disk."
uuid=$(blkid -s UUID -o value "$part")
[ -n "$uuid" ] || fail "Keel formatted the disk but could not read its id."
mkdir -p "$mp"
tmp=$(mktemp)
grep -v "[[:space:]]$mp[[:space:]]" /etc/fstab > "$tmp" || true
printf 'UUID=%s %s ext4 defaults,nofail,x-systemd.device-timeout=10s 0 2\n' "$uuid" "$mp" >> "$tmp"
cat "$tmp" > /etc/fstab
rm -f "$tmp"
systemctl daemon-reload >/dev/null 2>&1 || true
mount "$mp" >/dev/null 2>&1 || fail "Keel formatted the disk but could not open it."
chmod 0755 "$mp"
"#;

const OWNER_SCRIPT: &str = r#"set -uo pipefail
name="$1"; shift
fail() { echo "$1" >&2; exit 1; }
IFS= read -r pw || true
[ -n "$pw" ] || fail "No password was given."
if ! id "$name" >/dev/null 2>&1; then
  useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin --user-group "$name" >/dev/null 2>&1 || fail "Keel could not add that name."
fi
printf '%s\n%s\n' "$pw" "$pw" | smbpasswd -a -s "$name" >/dev/null 2>&1 || fail "The file sharing service did not accept that password."
smbpasswd -e "$name" >/dev/null 2>&1 || true
for d in "$@"; do
  [ -d "$d" ] && chown -R "$name:$name" "$d"
done
exit 0
"#;

const CHOWN_SCRIPT: &str = r#"set -uo pipefail
name="$1"; shift
for d in "$@"; do [ -d "$d" ] && chown "$name:$name" "$d"; done
exit 0
"#;

/// A new file or folder should belong to the person who opens the shares.
/// With no name chosen yet, it stays writable the same way the open share is.
pub fn give_to_owner(path: &Path) {
    if !cfg!(target_os = "linux") {
        return;
    }
    let text = path.to_string_lossy();
    if share_role(&text).is_none() {
        return;
    }
    let name = owner().unwrap_or_default();
    let _ = host(GRANT_SCRIPT, &[&name, text.as_ref()], None);
}

const GRANT_SCRIPT: &str = r#"set -uo pipefail
name="$1"
path="$2"
[ -e "$path" ] || exit 0
case "$path" in
  /srv/keel/*) ;;
  *) exit 0 ;;
esac
if [ -n "$name" ]; then
  chown -R -- "$name:$name" "$path"
else
  chmod -R a+rwX "$path"
fi
exit 0
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_section_names_stay_plain() {
        let rejected = |name| match add_section(name) {
            Err(message) => message,
            Ok(_) => panic!("{name} was accepted"),
        };
        assert!(rejected("").contains("letters"));
        assert!(rejected("files").contains("already used"));
        assert!(rejected("Copies").contains("already used"));
        assert!(rejected("A/B").contains("letters"));
    }

    #[test]
    fn owner_names_are_plain_and_not_system_accounts() {
        assert!(valid_owner_name("robby"));
        assert!(valid_owner_name("ro-by2"));
        assert!(!valid_owner_name("Robby"));
        assert!(!valid_owner_name("2robby"));
        assert!(!valid_owner_name("r"));
        assert!(!valid_owner_name("root"));
        assert!(!valid_owner_name("keel"));
        assert!(!valid_owner_name("rob by"));
    }

    #[test]
    fn share_role_reads_the_disk_from_the_path() {
        assert_eq!(share_role("/srv/keel/files/Documents").as_deref(), Some("files"));
        assert_eq!(share_role("/srv/keel/copy").as_deref(), Some("copy"));
        assert_eq!(share_role("/var/lib/keel/files"), None);
    }

    #[test]
    fn system_and_role_disks_cannot_be_reused() {
        let system = describe(Disk { name: "sda".into(), path: "/dev/sda".into(), kind: "disk".into(), system: true, mount: "/".into(), ..Disk::default() });
        assert!(!system.can_use);
        assert_eq!(system.holds, "Keel itself");
        let files = describe(Disk { name: "sdb".into(), path: "/dev/sdb".into(), kind: "disk".into(), label: "keel-files".into(), mount: "/srv/keel/files".into(), ..Disk::default() });
        assert!(!files.can_use);
        assert_eq!(files.role.as_deref(), Some("files"));
        let empty = describe(Disk { name: "sdc".into(), path: "/dev/sdc".into(), kind: "disk".into(), empty: true, ..Disk::default() });
        assert!(empty.can_use);
        assert_eq!(empty.holds, "Empty");
        let old = describe(Disk { name: "sdd".into(), path: "/dev/sdd".into(), kind: "disk".into(), fstype: "ntfs".into(), ..Disk::default() });
        assert!(old.can_use);
    }
}
