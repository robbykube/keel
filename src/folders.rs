//! Folders on this computer: look inside a share, make a folder, move, copy.
//!
//! Pictures are files in Media. This does not start a photo app.
//! On Linux only a mounted Keel disk (`/srv/keel/...`) can be opened, so a
//! missing disk is never written into. On Windows this serves sample folders
//! under the data directory so the page can be tried.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::model::Share;

#[derive(Serialize)]
pub struct FolderHome {
    pub sample: bool,
    pub message: String,
    pub folders: Vec<FolderInfo>,
}

#[derive(Serialize)]
pub struct FolderInfo {
    pub name: String,
    pub read_only: bool,
}

#[derive(Serialize)]
pub struct Listing {
    pub share: String,
    pub path: String,
    pub read_only: bool,
    pub entries: Vec<Entry>,
}

#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: &'static str,
    pub size: u64,
    pub modified_ms: u64,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub share: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Deserialize)]
pub struct MakeRequest {
    pub share: String,
    #[serde(default)]
    pub path: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct TransferRequest {
    pub from_share: String,
    #[serde(default)]
    pub from_path: String,
    pub to_share: String,
    #[serde(default)]
    pub to_path: String,
}

struct Root {
    name: String,
    path: PathBuf,
    read_only: bool,
}

const SAMPLE: &[(&str, bool)] = &[
    ("Documents", false),
    ("Work", false),
    ("Media", false),
    ("Software", false),
    ("Inbox", false),
    ("Copies", true),
];

pub fn home(shares: &[Share]) -> FolderHome {
    if cfg!(target_os = "linux") {
        let folders: Vec<FolderInfo> = linux_roots(shares)
            .into_iter()
            .map(|root| FolderInfo {
                name: root.name,
                read_only: root.read_only,
            })
            .collect();
        let message = if folders.is_empty() {
            "These folders appear after a disk is given a job. Open Disks and choose Use."
                .into()
        } else {
            String::new()
        };
        return FolderHome {
            sample: false,
            message,
            folders,
        };
    }
    let _ = ensure_sample();
    FolderHome {
        sample: true,
        message: "These are sample folders on this Windows copy. The Keel computer uses the real disks."
            .into(),
        folders: SAMPLE
            .iter()
            .map(|(name, read_only)| FolderInfo {
                name: (*name).into(),
                read_only: *read_only,
            })
            .collect(),
    }
}

pub fn list(shares: &[Share], share: &str, path: &str) -> Result<Listing, String> {
    list_root(&find_root(shares, share)?, path)
}

#[derive(Debug)]
pub struct Download {
    pub file: fs::File,
    pub name: String,
    pub len: u64,
}

pub fn open_download(shares: &[Share], share: &str, path: &str) -> Result<Download, String> {
    let root = find_root(shares, share)?;
    let rel = normalize(path)?;
    if rel.is_empty() {
        return Err("Choose a file to download.".into());
    }
    let full = resolve_existing(&root.path, &rel)?;
    let meta = fs::symlink_metadata(&full).map_err(|_| "That file is not there. Reload the page.".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("Keel does not open links. Use the real file.".into());
    }
    if meta.is_dir() {
        return Err("Open a folder to see what is in it. Download is for a file.".into());
    }
    if !meta.is_file() {
        return Err("Keel can only download a file.".into());
    }
    let name = full
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| safe_part(name))
        .ok_or_else(|| "That file name cannot be downloaded.".to_string())?
        .to_string();
    let file = fs::File::open(&full).map_err(|err| io_sentence(&err, "download that file"))?;
    Ok(Download {
        file,
        name,
        len: meta.len(),
    })
}

pub fn attachment_header(name: &str) -> String {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii() && c != '"' && c != '\\' && !c.is_control() {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

fn list_root(root: &Root, path: &str) -> Result<Listing, String> {
    let rel = normalize(path)?;
    let dir = resolve_existing(&root.path, &rel)?;
    let meta = fs::symlink_metadata(&dir).map_err(|_| "That folder is not there. Reload the page.".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("Keel does not open links. Use the real file.".into());
    }
    if !meta.is_dir() {
        return Err("That is a file, not a folder.".into());
    }
    let mut entries = Vec::new();
    let read = fs::read_dir(&dir).map_err(|err| io_sentence(&err, "open that folder"))?;
    for item in read {
        let item = item.map_err(|err| io_sentence(&err, "open that folder"))?;
        let name = item.file_name().to_string_lossy().to_string();
        if name == "lost+found" || !safe_part(&name) {
            continue;
        }
        let meta = match fs::symlink_metadata(item.path()) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let kind = if meta.file_type().is_symlink() {
            "link"
        } else if meta.is_dir() {
            "folder"
        } else if meta.is_file() {
            "file"
        } else {
            continue;
        };
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|time| time.as_millis() as u64)
            .unwrap_or(0);
        entries.push(Entry {
            name,
            kind,
            size: if kind == "file" { meta.len() } else { 0 },
            modified_ms,
        });
    }
    entries.sort_by(|a, b| {
        let rank = |kind: &str| match kind {
            "folder" => 0,
            "file" => 1,
            _ => 2,
        };
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(Listing {
        share: root.name.clone(),
        path: rel,
        read_only: root.read_only,
        entries,
    })
}

pub fn make_folder(shares: &[Share], share: &str, path: &str, name: &str) -> Result<PathBuf, String> {
    make_in(&find_root(shares, share)?, path, name)
}

fn make_in(root: &Root, path: &str, name: &str) -> Result<PathBuf, String> {
    if root.read_only {
        return Err(read_only_sentence());
    }
    let name = name.trim();
    if !valid_entry_name(name) {
        return Err("Use a plain name, without slashes.".into());
    }
    let rel = normalize(path)?;
    let parent = resolve_existing(&root.path, &rel)?;
    if !fs::symlink_metadata(&parent)
        .map(|meta| meta.is_dir() && !meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("That is a file, not a folder.".into());
    }
    let dest = parent.join(name);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(format!("Something named {name} is already in this folder."));
    }
    fs::create_dir(&dest).map_err(|err| io_sentence(&err, "make that folder"))?;
    Ok(dest)
}

pub fn copy_item(
    shares: &[Share],
    from_share: &str,
    from_path: &str,
    to_share: &str,
    to_path: &str,
) -> Result<PathBuf, String> {
    transfer(shares, from_share, from_path, to_share, to_path, false)
}

pub fn move_item(
    shares: &[Share],
    from_share: &str,
    from_path: &str,
    to_share: &str,
    to_path: &str,
) -> Result<PathBuf, String> {
    transfer(shares, from_share, from_path, to_share, to_path, true)
}

fn transfer(
    shares: &[Share],
    from_share: &str,
    from_path: &str,
    to_share: &str,
    to_path: &str,
    remove_source: bool,
) -> Result<PathBuf, String> {
    transfer_roots(
        &find_root(shares, from_share)?,
        from_path,
        &find_root(shares, to_share)?,
        to_path,
        remove_source,
    )
}

fn transfer_roots(
    from_root: &Root,
    from_path: &str,
    to_root: &Root,
    to_path: &str,
    remove_source: bool,
) -> Result<PathBuf, String> {
    if remove_source && from_root.read_only {
        return Err("This folder only keeps copies. Copy it into Documents, Work, or Media instead.".into());
    }
    if to_root.read_only {
        return Err(read_only_sentence());
    }
    let from_rel = normalize(from_path)?;
    if from_rel.is_empty() {
        return Err("Choose a file or folder inside, not the whole share.".into());
    }
    let to_rel = normalize(to_path)?;
    if from_root.name == to_root.name && inside(&from_rel, &to_rel) {
        return Err("Open a folder that is not inside this one.".into());
    }
    let source = resolve_existing(&from_root.path, &from_rel)?;
    let source_meta = fs::symlink_metadata(&source).map_err(|_| "That file is not there. Reload the page.".to_string())?;
    if source_meta.file_type().is_symlink() {
        return Err("Keel does not move links. Use the real file.".into());
    }
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Choose a file or folder.".to_string())?;
    let parent_rel = parent_of(&from_rel);
    if from_root.name == to_root.name && parent_rel == to_rel {
        return Err("It is already in this folder.".into());
    }
    let dest_parent = resolve_existing(&to_root.path, &to_rel)?;
    if !fs::symlink_metadata(&dest_parent)
        .map(|m| m.is_dir() && !m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("Open a folder to put it in.".into());
    }
    let dest = dest_parent.join(&name);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(format!("Something named {name} is already in that folder."));
    }
    if remove_source {
        match fs::rename(&source, &dest) {
            Ok(()) => return Ok(dest),
            Err(err) if crosses_devices(&err) => {}
            Err(err) => return Err(io_sentence(&err, "move that")),
        }
    }
    if let Err(err) = copy_tree(&source, &dest, 0) {
        let _ = remove_tree(&dest);
        return Err(err);
    }
    if remove_source && remove_tree(&source).is_err() {
        return Err(
            "The copy is in the new folder, but the original is still there. Delete the original if you meant to move it."
                .into(),
        );
    }
    Ok(dest)
}

fn linux_roots(shares: &[Share]) -> Vec<Root> {
    let mut roots = Vec::new();
    for share in shares {
        let Some(role) = crate::storage::share_role(&share.path) else {
            continue;
        };
        if !crate::storage::mounted(&role) {
            continue;
        }
        let path = PathBuf::from(&share.path);
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        if roots.iter().any(|root: &Root| root.name == share.name) {
            continue;
        }
        roots.push(Root {
            name: share.name.clone(),
            path,
            read_only: share.read_only,
        });
    }
    roots
}

fn find_root(shares: &[Share], share: &str) -> Result<Root, String> {
    let name = share.trim();
    if name.is_empty() {
        return Err("Choose a folder on the left first.".into());
    }
    if cfg!(target_os = "linux") {
        return linux_roots(shares)
            .into_iter()
            .find(|root| root.name == name)
            .ok_or_else(|| {
                "That folder is not open. If its disk was unplugged, check the cable. Keel opens it again by itself."
                    .into()
            });
    }
    let (label, read_only) = SAMPLE
        .iter()
        .find(|(label, _)| *label == name)
        .copied()
        .ok_or_else(|| "Choose a folder on the left.".to_string())?;
    Ok(Root {
        name: label.into(),
        path: ensure_sample().join(label),
        read_only,
    })
}

fn sample_root() -> PathBuf {
    let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("mock-files")
}

fn ensure_sample() -> PathBuf {
    let root = sample_root();
    if root.join("Documents").is_dir() {
        return root;
    }
    for (name, _) in SAMPLE {
        let _ = fs::create_dir_all(root.join(name));
    }
    let _ = fs::write(
        root.join("Documents").join("Read me.txt"),
        b"This is a sample file on this Windows copy.\n",
    );
    let _ = fs::create_dir_all(root.join("Media").join("Photos"));
    let _ = fs::write(
        root.join("Copies").join("Yesterday.txt"),
        b"A sample of an older copy.\n",
    );
    root
}

fn normalize(path: &str) -> Result<String, String> {
    let parts = parts(path)?;
    Ok(parts.join("/"))
}

fn parts(path: &str) -> Result<Vec<&str>, String> {
    let path = path.trim().trim_matches(['/', '\\']);
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in path.split(['/', '\\']) {
        if !safe_part(part) {
            return Err("That place is outside this folder.".into());
        }
        out.push(part);
    }
    Ok(out)
}

fn safe_part(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part != "lost+found"
        && part.len() <= 255
        && !part.chars().any(|c| c.is_control() || c == '/' || c == '\\' || c == ':')
}

pub fn valid_entry_name(name: &str) -> bool {
    let name = name.trim();
    if !safe_part(name) || name.len() > 200 {
        return false;
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return false;
    }
    if name.chars().any(|c| matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "COM1" | "COM2" | "LPT1" | "LPT2"
    )
}

fn resolve_existing(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let root_meta = fs::symlink_metadata(root).map_err(|_| "That folder is not there. Reload the page.".to_string())?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err("That folder is not there. Reload the page.".into());
    }
        let mut cur = root.to_path_buf();
    for part in parts(rel)? {
        if Path::new(part).is_absolute() {
            return Err("That place is outside this folder.".into());
        }
        cur.push(part);
        if !cur.starts_with(root) {
            return Err("That place is outside this folder.".into());
        }
        let meta = fs::symlink_metadata(&cur).map_err(|_| "That folder is not there. Reload the page.".to_string())?;
        if meta.file_type().is_symlink() {
            return Err("Keel does not open links. Use the real file.".into());
        }
    }
    Ok(cur)
}

fn parent_of(rel: &str) -> String {
    match rel.rfind('/') {
        Some(index) => rel[..index].to_string(),
        None => String::new(),
    }
}

fn inside(folder: &str, path: &str) -> bool {
    !folder.is_empty() && (path == folder || path.starts_with(&(folder.to_string() + "/")))
}

fn copy_tree(from: &Path, to: &Path, depth: u32) -> Result<(), String> {
    if depth > 64 {
        return Err("That folder is nested too deeply to copy here.".into());
    }
    let meta = fs::symlink_metadata(from).map_err(|err| io_sentence(&err, "copy that"))?;
    if meta.file_type().is_symlink() {
        return Err("Keel does not copy links. Use the real file.".into());
    }
    if meta.is_dir() {
        fs::create_dir(to).map_err(|err| io_sentence(&err, "copy that"))?;
        let read = fs::read_dir(from).map_err(|err| io_sentence(&err, "copy that"))?;
        for item in read {
            let item = item.map_err(|err| io_sentence(&err, "copy that"))?;
            copy_tree(&item.path(), &to.join(item.file_name()), depth + 1)?;
        }
        return Ok(());
    }
    if meta.is_file() {
        fs::copy(from, to).map(|_| ()).map_err(|err| io_sentence(&err, "copy that"))?;
        return Ok(());
    }
    Err("Keel can only copy files and folders.".into())
}

fn remove_tree(path: &Path) -> Result<(), String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(io_sentence(&err, "remove the original")),
    };
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path).map_err(|err| io_sentence(&err, "remove the original"))?;
    } else if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|err| io_sentence(&err, "remove the original"))?;
    }
    Ok(())
}

fn crosses_devices(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::CrossesDevices || err.raw_os_error() == Some(18)
}

fn read_only_sentence() -> String {
    "This folder only keeps copies. Open Documents, Work, or Media to change files.".into()
}

fn io_sentence(err: &std::io::Error, action: &str) -> String {
    match err.kind() {
        ErrorKind::AlreadyExists => "Something with that name is already in this folder.".into(),
        ErrorKind::NotFound => "That folder is not there. Reload the page.".into(),
        ErrorKind::PermissionDenied => {
            format!("Keel could not {action}. Check that the disk is connected, then try again.")
        }
        _ => format!("Keel could not {action}. Try again."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "keel-folders-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn root(path: PathBuf, read_only: bool) -> Root {
        Root {
            name: "Documents".into(),
            path,
            read_only,
        }
    }

    #[test]
    fn names_are_plain() {
        assert!(valid_entry_name("Bills and Taxes"));
        assert!(valid_entry_name("Photos"));
        assert!(!valid_entry_name(""));
        assert!(!valid_entry_name(".."));
        assert!(!valid_entry_name("a/b"));
        assert!(!valid_entry_name("a\\b"));
        assert!(!valid_entry_name("CON"));
        assert!(!valid_entry_name("note."));
    }

    #[test]
    fn paths_cannot_leave_the_folder() {
        assert!(parts("Photos/2024").is_ok());
        assert!(parts("../secret").is_err());
        assert!(parts("Photos/../../etc").is_err());
        assert_eq!(parts("/etc/passwd").unwrap().join("/"), "etc/passwd");
        assert!(parts("C:/Windows").is_err());
        assert!(inside("Photos", "Photos/2024"));
        assert!(!inside("Photos", "Photos-old"));
        assert!(!inside("", "Photos"));
    }

    #[test]
    fn make_list_copy_and_move_stay_inside() {
        let dir = scratch();
        let docs = root(dir.join("Documents"), false);
        let work = Root {
            name: "Work".into(),
            path: dir.join("Work"),
            read_only: false,
        };
        fs::create_dir_all(&docs.path).unwrap();
        fs::create_dir_all(&work.path).unwrap();
        fs::write(docs.path.join("Note.txt"), b"hello").unwrap();

        make_in(&docs, "", "Bills and Taxes").unwrap();
        let listed = list_root(&docs, "").unwrap();
        assert_eq!(listed.entries.len(), 2);
        assert_eq!(listed.entries[0].name, "Bills and Taxes");
        assert_eq!(listed.entries[0].kind, "folder");
        assert!(list_root(&docs, "../secret").is_err());
        assert!(make_in(&docs, "", "../nope").is_err());

        let copied = transfer_roots(&docs, "Note.txt", &work, "", false).unwrap();
        assert_eq!(fs::read(copied).unwrap(), b"hello");
        assert!(docs.path.join("Note.txt").exists());
        assert!(transfer_roots(&docs, "Note.txt", &work, "", true).is_err());

        fs::write(docs.path.join("Move me.txt"), b"move").unwrap();
        transfer_roots(&docs, "Move me.txt", &work, "", true).unwrap();
        assert!(!docs.path.join("Move me.txt").exists());
        assert!(work.path.join("Move me.txt").exists());
        assert!(transfer_roots(&work, "Move me.txt", &work, "", true).is_err());

        make_in(&docs, "", "Photos").unwrap();
        make_in(&docs, "Photos", "2024").unwrap();
        fs::write(docs.path.join("Photos").join("pic.txt"), b"pic").unwrap();
        assert!(transfer_roots(&docs, "Photos", &docs, "Photos/2024", true).is_err());
        assert!(docs.path.join("Photos").join("pic.txt").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_only_folder_can_be_copied_from_and_not_changed() {
        let dir = scratch();
        let copies = Root {
            name: "Copies".into(),
            path: dir.join("Copies"),
            read_only: true,
        };
        let docs = root(dir.join("Documents"), false);
        fs::create_dir_all(&copies.path).unwrap();
        fs::create_dir_all(&docs.path).unwrap();
        fs::write(copies.path.join("Yesterday.txt"), b"old").unwrap();

        assert!(make_in(&copies, "", "New").is_err());
        assert!(transfer_roots(&copies, "Yesterday.txt", &docs, "", true).is_err());
        assert!(copies.path.join("Yesterday.txt").exists());
        let copied = transfer_roots(&copies, "Yesterday.txt", &docs, "", false).unwrap();
        assert_eq!(fs::read(copied).unwrap(), b"old");
        assert!(transfer_roots(&docs, "Yesterday.txt", &copies, "", false).is_err());

        let opened = open_download_root(&copies, "Yesterday.txt").unwrap();
        assert_eq!(opened.name, "Yesterday.txt");
        assert_eq!(opened.len, 3);
        assert!(open_download_root(&copies, "").is_err());
        assert!(open_download_root(&copies, "../secret").is_err());
        fs::create_dir(copies.path.join("Older")).unwrap();
        assert!(open_download_root(&copies, "Older").unwrap_err().contains("folder"));
        assert!(attachment_header("Note.txt").contains("filename=\"Note.txt\""));
        assert!(!attachment_header("a\"b\r\n").contains('\n'));
        assert!(!attachment_header("a\"b\r\n").contains('\r'));

        let _ = fs::remove_dir_all(&dir);
    }

    fn open_download_root(root: &Root, path: &str) -> Result<Download, String> {
        let rel = normalize(path)?;
        if rel.is_empty() {
            return Err("Choose a file to download.".into());
        }
        let full = resolve_existing(&root.path, &rel)?;
        let meta = fs::symlink_metadata(&full).map_err(|_| "missing".to_string())?;
        if meta.file_type().is_symlink() {
            return Err("Keel does not open links. Use the real file.".into());
        }
        if meta.is_dir() {
            return Err("Open a folder to see what is in it. Download is for a file.".into());
        }
        let name = full.file_name().unwrap().to_string_lossy().to_string();
        let file = fs::File::open(&full).unwrap();
        Ok(Download { file, name, len: meta.len() })
    }
}
