//! When each picture was taken.
//!
//! A camera writes the moment into the picture itself. Reading it means
//! opening every file, which is slow on a spinning disk, so Keel keeps what it
//! read in `.keel/dates.tsv` beside the copies, and reads new pictures in the
//! background. Until then a picture counts by the date on its file.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// How much of a file to read looking for the date. Cameras put it first.
const HEAD: u64 = 256 * 1024;
/// Save what was read every so often, so a restart does not start over.
const SAVE_EVERY: usize = 500;

/// Picture path → (file modified, file size, taken).
type Entries = HashMap<String, (i64, u64, i64)>;

static INDEX: Mutex<Option<HashMap<PathBuf, Entries>>> = Mutex::new(None);
static FILLING: AtomicBool = AtomicBool::new(false);

/// One picture to look up: its path in the library, the file on the disk,
/// and the file's modified time and size.
pub struct Wanted {
    pub rel: String,
    pub file: PathBuf,
    pub modified: i64,
    pub len: u64,
}

/// The taken time for each picture, in order, when it has been read. Pictures
/// not read yet, or changed since, are read in the background; the answer
/// says whether every one was known.
pub fn look_up(media: &Path, wanted: Vec<Wanted>) -> (Vec<Option<i64>>, bool) {
    let mut found = Vec::with_capacity(wanted.len());
    let mut missing = Vec::new();
    {
        let mut guard = INDEX.lock().unwrap_or_else(|poison| poison.into_inner());
        let entries = entries_for(&mut guard, media);
        for item in wanted {
            match entries.get(&item.rel) {
                Some(&(modified, len, taken)) if modified == item.modified && len == item.len => {
                    found.push(Some(taken));
                }
                _ => {
                    found.push(None);
                    missing.push(item);
                }
            }
        }
    }
    let complete = missing.is_empty();
    if !complete && !FILLING.swap(true, Ordering::AcqRel) {
        let media = media.to_path_buf();
        std::thread::spawn(move || {
            crate::gallery::lower_priority();
            fill_now(&media, missing);
            FILLING.store(false, Ordering::Release);
        });
    }
    (found, complete)
}

/// Reads the taken time of each picture and saves it.
pub fn fill_now(media: &Path, missing: Vec<Wanted>) {
    let mut pending = Vec::new();
    for item in missing {
        let taken = taken_in(&item.file).unwrap_or(item.modified);
        pending.push((item.rel, (item.modified, item.len, taken)));
        if pending.len() >= SAVE_EVERY {
            store(media, std::mem::take(&mut pending));
        }
    }
    store(media, pending);
}

/// Waits for reading in the background to finish.
#[cfg(test)]
pub fn wait_idle() {
    while FILLING.load(Ordering::Acquire) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Forgets pictures that are no longer in the library.
pub fn prune(media: &Path, keep: &HashSet<String>) {
    let mut guard = INDEX.lock().unwrap_or_else(|poison| poison.into_inner());
    let entries = entries_for(&mut guard, media);
    let before = entries.len();
    entries.retain(|rel, _| keep.contains(rel));
    if entries.len() != before {
        save(media, entries);
    }
}

fn store(media: &Path, pending: Vec<(String, (i64, u64, i64))>) {
    if pending.is_empty() {
        return;
    }
    let mut guard = INDEX.lock().unwrap_or_else(|poison| poison.into_inner());
    let entries = entries_for(&mut guard, media);
    entries.extend(pending);
    save(media, entries);
}

fn entries_for<'a>(guard: &'a mut Option<HashMap<PathBuf, Entries>>, media: &Path) -> &'a mut Entries {
    guard
        .get_or_insert_with(HashMap::new)
        .entry(media.to_path_buf())
        .or_insert_with(|| load(media))
}

fn index_file(media: &Path) -> PathBuf {
    media.parent().unwrap_or(media).join(".keel").join("dates.tsv")
}

fn load(media: &Path) -> Entries {
    let mut entries = Entries::new();
    let Ok(text) = fs::read_to_string(index_file(media)) else {
        return entries;
    };
    for line in text.lines() {
        let mut parts = line.split('\t');
        let (Some(rel), Some(modified), Some(len), Some(taken), None) =
            (parts.next(), parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (Ok(modified), Ok(len), Ok(taken)) = (modified.parse(), len.parse(), taken.parse()) else {
            continue;
        };
        entries.insert(rel.to_string(), (modified, len, taken));
    }
    entries
}

fn save(media: &Path, entries: &Entries) {
    let path = index_file(media);
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let mut text = String::with_capacity(entries.len() * 64);
    for (rel, (modified, len, taken)) in entries {
        if rel.contains(['\t', '\n', '\r']) {
            continue;
        }
        text.push_str(&format!("{rel}\t{modified}\t{len}\t{taken}\n"));
    }
    let tmp = path.with_extension("part");
    if fs::write(&tmp, text).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

/// The moment a camera wrote into a JPEG picture, in seconds, counted as if
/// the camera's clock were UTC. Other kinds of file answer None.
pub fn taken_in(path: &Path) -> Option<i64> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if ext != "jpg" && ext != "jpeg" {
        return None;
    }
    let mut head = Vec::new();
    fs::File::open(path).ok()?.take(HEAD).read_to_end(&mut head).ok()?;
    taken_in_jpeg(&head)
}

fn taken_in_jpeg(bytes: &[u8]) -> Option<i64> {
    if bytes.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xD9 || marker == 0xDA {
            return None;
        }
        let len = u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]) as usize;
        if len < 2 {
            return None;
        }
        let start = at + 4;
        let end = (at + 2 + len).min(bytes.len());
        if marker == 0xE1 && bytes.get(start..start + 6) == Some(b"Exif\0\0") {
            if let Some(taken) = taken_in_tiff(&bytes[start + 6..end]) {
                return Some(taken);
            }
        }
        at += 2 + len;
    }
    None
}

fn taken_in_tiff(tiff: &[u8]) -> Option<i64> {
    let little = match tiff.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let b = tiff.get(at..at + 2)?;
        Some(if little { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b = tiff.get(at..at + 4)?;
        let b = [b[0], b[1], b[2], b[3]];
        Some(if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    // Where a tag's entry is in a directory.
    let find = |ifd: usize, tag: u16| -> Option<usize> {
        let count = u16_at(ifd)? as usize;
        (0..count.min(512))
            .map(|n| ifd + 2 + n * 12)
            .find(|&entry| u16_at(entry) == Some(tag))
    };
    let text_at = |entry: usize| -> Option<i64> {
        if u16_at(entry + 2)? != 2 {
            return None;
        }
        let count = u32_at(entry + 4)? as usize;
        let at = if count <= 4 { entry + 8 } else { u32_at(entry + 8)? as usize };
        parse_moment(tiff.get(at..at + count.min(32))?)
    };
    let first = u32_at(4)? as usize;
    if let Some(pointer) = find(first, 0x8769) {
        let exif = u32_at(pointer + 8)? as usize;
        for tag in [0x9003, 0x9004] {
            if let Some(taken) = find(exif, tag).and_then(text_at) {
                return Some(taken);
            }
        }
    }
    find(first, 0x0132).and_then(text_at)
}

/// "2024:07:21 18:03:55" as seconds.
fn parse_moment(raw: &[u8]) -> Option<i64> {
    let text = std::str::from_utf8(raw).ok()?.trim_end_matches('\0').trim();
    let text = text.get(0..19)?;
    let b = text.as_bytes();
    if b[4] != b':' || b[7] != b':' || b[10] != b' ' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1900..=2200).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

/// Days since 1970-01-01 for a calendar date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A JPEG start with a camera date, as a phone writes it.
    pub(crate) fn jpeg_with_date(little: bool, moment: &str) -> Vec<u8> {
        let p16 = |v: u16| if little { v.to_le_bytes().to_vec() } else { v.to_be_bytes().to_vec() };
        let p32 = |v: u32| if little { v.to_le_bytes().to_vec() } else { v.to_be_bytes().to_vec() };
        let mut tiff = Vec::new();
        tiff.extend(if little { b"II".to_vec() } else { b"MM".to_vec() });
        tiff.extend(p16(42));
        tiff.extend(p32(8));
        // First directory: one entry pointing at the camera directory at 26.
        tiff.extend(p16(1));
        tiff.extend(p16(0x8769));
        tiff.extend(p16(4));
        tiff.extend(p32(1));
        tiff.extend(p32(26));
        tiff.extend(p32(0));
        // Camera directory: DateTimeOriginal, text stored at 44.
        tiff.extend(p16(1));
        tiff.extend(p16(0x9003));
        tiff.extend(p16(2));
        tiff.extend(p32(20));
        tiff.extend(p32(44));
        tiff.extend(p32(0));
        tiff.extend(moment.as_bytes());
        tiff.push(0);
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend(tiff);
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpeg.extend(((app1.len() + 2) as u16).to_be_bytes());
        jpeg.extend(app1);
        jpeg.extend([0xFF, 0xD9]);
        jpeg
    }

    #[test]
    fn the_camera_date_is_read_either_way_round() {
        let want = days_from_civil(2024, 7, 21) * 86_400 + 18 * 3600 + 3 * 60 + 55;
        assert_eq!(taken_in_jpeg(&jpeg_with_date(true, "2024:07:21 18:03:55")), Some(want));
        assert_eq!(taken_in_jpeg(&jpeg_with_date(false, "2024:07:21 18:03:55")), Some(want));
        assert_eq!(taken_in_jpeg(&jpeg_with_date(true, "0000:00:00 00:00:00")), None);
        assert_eq!(taken_in_jpeg(b"\xFF\xD8\xFF\xD9"), None);
        assert_eq!(taken_in_jpeg(b"not a picture"), None);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }

    #[test]
    fn dates_are_kept_and_forgotten() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-dates-{stamp}"));
        let media = disk.join("Media");
        fs::create_dir_all(&media).unwrap();
        let file = media.join("a.jpg");
        fs::write(&file, jpeg_with_date(true, "2020:01:02 03:04:05")).unwrap();
        let wanted = || vec![Wanted { rel: "Pictures/a.jpg".into(), file: file.clone(), modified: 7, len: 9 }];
        fill_now(&media, wanted());
        let (found, complete) = look_up(&media, wanted());
        assert!(complete);
        assert_eq!(found, vec![Some(days_from_civil(2020, 1, 2) * 86_400 + 3 * 3600 + 4 * 60 + 5)]);
        assert!(fs::read_to_string(disk.join(".keel").join("dates.tsv")).unwrap().contains("Pictures/a.jpg\t"));
        prune(&media, &HashSet::new());
        assert!(!fs::read_to_string(disk.join(".keel").join("dates.tsv")).unwrap().contains("Pictures/a.jpg"));
        // A changed file is read again.
        fill_now(&media, wanted());
        let (_, complete) = look_up(&media, vec![Wanted { rel: "Pictures/a.jpg".into(), file: file.clone(), modified: 8, len: 9 }]);
        assert!(!complete);
        let _ = fs::remove_dir_all(&disk);
    }
}
