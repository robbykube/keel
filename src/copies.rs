//! The daily copy: Files to the nightly-copy disk, and Documents and Work to
//! the third-copy disk. A file that changed or disappeared is moved to
//! `Older/<date>/` on the copy disk first, and kept for 30 days, so a
//! mistake on the files disk can be undone from the copy.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage::{self, mount_point, CopyStatus};

const KEEP_DAYS: i64 = 30;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const TMP_PREFIX: &str = ".keel-copy-";

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// The files disk and at least one copy disk are open.
pub fn can_run() -> bool {
    storage::mounted("files") && (storage::mounted("copy") || storage::mounted("extra"))
}

pub fn can_copy_computer() -> bool {
    storage::mounted("copy") || storage::mounted("extra")
}

pub fn computer_status() -> CopyStatus {
    storage::computer_copy_status()
}

const COMPUTER_COPY_NEEDS_DISK: &str =
    "Give a disk the Nightly copy job. Keel keeps a copy of itself there, apart from the files.";

/// Copy the program, the people, and the settings onto the Nightly copy disk,
/// and onto the Third copy disk when that one is connected. The folder is
/// "This computer", beside the file mirror, not inside it.
pub fn copy_computer() -> Result<String, String> {
    if !can_copy_computer() {
        return Err(COMPUTER_COPY_NEEDS_DISK.into());
    }
    let nightly = storage::mounted("copy");
    let third = storage::mounted("extra");
    let done = match (nightly, third) {
        (true, true) => {
            "This computer is copied onto the Nightly copy disk and the Third copy disk, apart from the files."
        }
        (false, true) => "This computer is copied onto the Third copy disk, apart from the files.",
        _ => "This computer is copied onto the Nightly copy disk, apart from the files.",
    };
    storage::set_computer_copy_status(CopyStatus {
        last_run_ms: storage::now_ms(),
        last_ok_ms: storage::computer_copy_status().last_ok_ms,
        ok: false,
        message: "Copying this computer.".into(),
    });
    match storage::host("exec /bin/bash -s", &[], Some(COMPUTER_COPY_SCRIPT)) {
        Ok(()) => {
            storage::set_computer_copy_status(CopyStatus {
                last_run_ms: storage::now_ms(),
                last_ok_ms: storage::now_ms(),
                ok: true,
                message: done.into(),
            });
            Ok(done.into())
        }
        Err(error) => {
            storage::set_computer_copy_status(CopyStatus {
                last_run_ms: storage::now_ms(),
                last_ok_ms: storage::computer_copy_status().last_ok_ms,
                ok: false,
                message: error.clone(),
            });
            Err(error)
        }
    }
}

/// Runs as root, outside Keel's sandbox. Destinations are only the two copy
/// disks, and only while those disks are actually mounted.
const COMPUTER_COPY_SCRIPT: &str = r#"set -euo pipefail
copy=/srv/keel/copy
extra=/srv/keel/extra
stage=$(mktemp -d /tmp/keel-computer.XXXXXX)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/program" "$stage/people"
cp -a /usr/bin/keel "$stage/program/keel"
cp -a /usr/lib/keel/web "$stage/program/web"
if [ -d /var/lib/keel ]; then
  tar -C /var/lib/keel --exclude=./files --exclude=./updates/staged --exclude=./updates/previous -cf - . | tar -C "$stage/people" -xf -
fi
day=$(date +%F)
copied=0
for root in "$copy" "$extra"; do
  if ! mountpoint -q "$root"; then
    continue
  fi
  dest="$root/This computer"
  mkdir -p "$dest/Older"
  rm -rf "$dest/Latest"
  mkdir -p "$dest/Latest"
  cp -a "$stage/." "$dest/Latest/"
  rm -rf "$dest/Older/$day"
  mkdir -p "$dest/Older/$day"
  cp -a "$stage/." "$dest/Older/$day/"
  chmod -R go-rwx "$dest"
  find "$dest/Older" -mindepth 1 -maxdepth 1 -type d -mtime +30 -exec rm -rf {} +
  copied=1
done
if [ "$copied" -ne 1 ]; then
  echo "Give a disk the Nightly copy job. Keel keeps a copy of itself there, apart from the files." >&2
  exit 1
fi
sync
"#;

pub fn due() -> bool {
    can_run() && !running() && storage::now_ms().saturating_sub(storage::copy_status().last_run_ms) >= DAY_MS
}

#[derive(Default)]
struct Tally {
    copied: u64,
    kept: u64,
    errors: Vec<String>,
}

impl Tally {
    fn error(&mut self, path: &Path, error: impl std::fmt::Display) {
        if self.errors.len() < 20 {
            self.errors.push(format!("{}: {error}", path.display()));
        } else {
            self.errors.push(String::new());
        }
    }
}

/// Run every copy job now. Returns `None` if a copy is already running.
pub fn run_now() -> Option<CopyStatus> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return None;
    }
    let status = run_jobs();
    storage::set_copy_status(status.clone());
    RUNNING.store(false, Ordering::SeqCst);
    Some(status)
}

fn run_jobs() -> CopyStatus {
    let started = storage::now_ms();
    let mut previous = storage::copy_status();
    if !can_run() {
        previous.last_run_ms = started;
        previous.ok = false;
        previous.message = "The files disk or the copy disks are not connected.".into();
        return previous;
    }
    let today = date(today_days());
    let files = mount_point("files");
    let mut tally = Tally::default();
    let mut done = Vec::new();

    if storage::mounted("copy") {
        let root = mount_point("copy");
        mirror(&files, &root.join("Latest"), &root.join("Older").join(&today), &mut tally);
        prune(&root.join("Older"), &mut tally);
        done.push("the nightly copy");
    }
    if storage::mounted("extra") {
        let root = mount_point("extra");
        for folder in ["Documents", "Work"] {
            let source = files.join(folder);
            if source.is_dir() {
                mirror(
                    &source,
                    &root.join("Latest").join(folder),
                    &root.join("Older").join(&today).join(folder),
                    &mut tally,
                );
            }
        }
        prune(&root.join("Older"), &mut tally);
        done.push("the third copy");
    }

    let ok = tally.errors.is_empty();
    let mut message = format!(
        "Copied {} changed files to {}. {} older versions kept.",
        tally.copied,
        done.join(" and "),
        tally.kept
    );
    if !ok {
        let shown: Vec<&str> = tally
            .errors
            .iter()
            .map(String::as_str)
            .filter(|e| !e.is_empty())
            .take(3)
            .collect();
        message = format!(
            "{message} {} files could not be copied, for example {}",
            tally.errors.len(),
            shown.join("; ")
        );
    }
    CopyStatus {
        last_run_ms: started,
        last_ok_ms: if ok { storage::now_ms() } else { previous.last_ok_ms },
        ok,
        message,
    }
}

fn mirror(source: &Path, target: &Path, older: &Path, tally: &mut Tally) {
    if let Err(error) = fs::create_dir_all(target) {
        tally.error(target, error);
        return;
    }
    copy_dir(source, target, older, true, tally);
    remove_extra(source, target, older, true, tally);
}

fn skipped(name: &str, top: bool) -> bool {
    (top && name == "lost+found") || name.starts_with(TMP_PREFIX)
}

fn copy_dir(source: &Path, target: &Path, older: &Path, top: bool, tally: &mut Tally) {
    let entries = match fs::read_dir(source) {
        Ok(entries) => entries,
        Err(error) => return tally.error(source, error),
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if skipped(&name.to_string_lossy(), top) {
            continue;
        }
        let from = entry.path();
        let to = target.join(&name);
        let keep = older.join(&name);
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if to.exists() && !to.is_dir() {
                keep_old(&to, &keep, tally);
            }
            if !to.is_dir() {
                if let Err(error) = fs::create_dir(&to) {
                    tally.error(&to, error);
                    continue;
                }
                same_owner(&from, &to);
            }
            copy_dir(&from, &to, &keep, false, tally);
        } else if kind.is_file() {
            let Ok(meta) = entry.metadata() else { continue };
            if let Ok(existing) = fs::symlink_metadata(&to) {
                if existing.is_file()
                    && existing.len() == meta.len()
                    && seconds(existing.modified().ok()) == seconds(meta.modified().ok())
                {
                    continue;
                }
                keep_old(&to, &keep, tally);
            }
            match copy_file(&from, &to, &meta) {
                Ok(()) => tally.copied += 1,
                Err(error) => tally.error(&from, error),
            }
        }
    }
}

fn remove_extra(source: &Path, target: &Path, older: &Path, top: bool, tally: &mut Tally) {
    let entries = match fs::read_dir(target) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let to = entry.path();
        if name.to_string_lossy().starts_with(TMP_PREFIX) {
            let _ = fs::remove_file(&to);
            continue;
        }
        if top && name == "lost+found" {
            continue;
        }
        let from = source.join(&name);
        let keep = older.join(&name);
        match fs::symlink_metadata(&from) {
            Err(_) => keep_old(&to, &keep, tally),
            Ok(meta) if meta.file_type().is_symlink() => keep_old(&to, &keep, tally),
            Ok(meta) => {
                if meta.is_dir() && to.is_dir() {
                    remove_extra(&from, &to, &keep, false, tally);
                }
            }
        }
    }
}

fn copy_file(from: &Path, to: &Path, meta: &fs::Metadata) -> io::Result<()> {
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let tmp = to.with_file_name(format!("{TMP_PREFIX}{name}"));
    let result = (|| {
        fs::copy(from, &tmp)?;
        if let Ok(modified) = meta.modified() {
            fs::OpenOptions::new().write(true).open(&tmp)?.set_modified(modified)?;
        }
        same_owner(from, &tmp);
        fs::rename(&tmp, to)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn keep_old(path: &Path, keep: &Path, tally: &mut Tally) {
    if let Some(parent) = keep.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            return tally.error(parent, error);
        }
    }
    if fs::symlink_metadata(keep).is_ok() {
        let _ = if keep.is_dir() { fs::remove_dir_all(keep) } else { fs::remove_file(keep) };
    }
    match fs::rename(path, keep) {
        Ok(()) => tally.kept += 1,
        Err(error) => tally.error(path, error),
    }
}

fn prune(older: &Path, tally: &mut Tally) {
    let today = today_days();
    let Ok(entries) = fs::read_dir(older) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(day) = parse_date(&name) else { continue };
        if today - day > KEEP_DAYS {
            if let Err(error) = fs::remove_dir_all(entry.path()) {
                tally.error(&entry.path(), error);
            }
        }
    }
}

#[cfg(unix)]
fn same_owner(from: &Path, to: &Path) {
    use std::os::unix::fs::MetadataExt;
    if let Ok(meta) = fs::metadata(from) {
        let _ = std::os::unix::fs::chown(to, Some(meta.uid()), Some(meta.gid()));
    }
}

#[cfg(not(unix))]
fn same_owner(_from: &Path, _to: &Path) {}

fn seconds(time: Option<SystemTime>) -> Option<u64> {
    time.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs())
}

fn today_days() -> i64 {
    (storage::now_ms() / DAY_MS) as i64
}

/// `YYYY-MM-DD` for a count of days since 1970-01-01.
fn date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn parse_date(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(20_740), "2026-10-14");
        assert_eq!(parse_date("2026-10-14"), Some(20_740));
        assert_eq!(parse_date("2024-02-29").map(date).as_deref(), Some("2024-02-29"));
        assert_eq!(parse_date("notes"), None);
    }

    #[test]
    fn mirror_copies_changes_and_keeps_old_versions() {
        let base = std::env::temp_dir().join(format!("keel-copies-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("files");
        let dst = base.join("copy/Latest");
        let older = base.join("copy/Older/2026-10-14");
        fs::create_dir_all(src.join("Documents")).unwrap();
        fs::create_dir_all(src.join("lost+found")).unwrap();
        fs::write(src.join("Documents/a.txt"), "one").unwrap();
        fs::write(src.join("Documents/b.txt"), "bee").unwrap();

        let mut tally = Tally::default();
        mirror(&src, &dst, &older, &mut tally);
        assert!(tally.errors.is_empty(), "{:?}", tally.errors);
        assert_eq!(tally.copied, 2);
        assert_eq!(fs::read_to_string(dst.join("Documents/a.txt")).unwrap(), "one");
        assert!(!dst.join("lost+found").exists());

        let mut again = Tally::default();
        mirror(&src, &dst, &older, &mut again);
        assert_eq!(again.copied, 0);

        fs::write(src.join("Documents/a.txt"), "two!").unwrap();
        fs::remove_file(src.join("Documents/b.txt")).unwrap();
        let mut changed = Tally::default();
        mirror(&src, &dst, &older, &mut changed);
        assert_eq!(changed.copied, 1);
        assert_eq!(changed.kept, 2);
        assert_eq!(fs::read_to_string(dst.join("Documents/a.txt")).unwrap(), "two!");
        assert_eq!(fs::read_to_string(older.join("Documents/a.txt")).unwrap(), "one");
        assert_eq!(fs::read_to_string(older.join("Documents/b.txt")).unwrap(), "bee");
        assert!(!dst.join("Documents/b.txt").exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn computer_copy_stays_beside_the_file_mirror() {
        assert!(COMPUTER_COPY_SCRIPT.contains("This computer"));
        assert!(COMPUTER_COPY_SCRIPT.contains("/srv/keel/copy"));
        assert!(COMPUTER_COPY_SCRIPT.contains("/srv/keel/extra"));
        assert!(COMPUTER_COPY_SCRIPT.contains("--exclude=./files"));
        assert!(!COMPUTER_COPY_SCRIPT.contains("/srv/keel/files"));
        assert!(!COMPUTER_COPY_SCRIPT.contains("rsync"));
        if !can_copy_computer() {
            let err = copy_computer().unwrap_err();
            assert_eq!(err, COMPUTER_COPY_NEEDS_DISK);
        }
    }
}
