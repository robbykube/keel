//! The light Photos page. It shows picture files already in Media.
//! The library may be named Photos or Pictures. Nothing is downloaded,
//! and the pictures stay on the disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use sha2::{Digest, Sha256};

static PREVIEW_GATE: Mutex<()> = Mutex::new(());

const PICTURE_LIMIT: usize = 400;
const WALK_DEPTH: u32 = 6;
const LIBRARY: &[&str] = &["Photos", "Pictures"];

#[derive(Serialize)]
pub struct Picture {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct Album {
    pub pictures: Vec<Picture>,
    pub message: String,
}

pub struct Opened {
    pub file: fs::File,
    pub len: u64,
    pub kind: &'static str,
}

pub fn album() -> Album {
    let pictures = library(&media_dir());
    let message = if pictures.is_empty() {
        "Pictures live in Media. None are there yet.".into()
    } else {
        "Pictures already in Media. They stay on the disk. Small copies stay on the disk with them.".into()
    };
    Album { pictures, message }
}

pub fn open_small(rel: &str) -> Result<Opened, String> {
    open_small_in(&media_dir(), rel)
}

pub fn open_small_in(media: &Path, rel: &str) -> Result<Opened, String> {
    if !in_library(rel) {
        return Err("That picture is not in Media.".into());
    }
    let source = file_in(media, rel)?;
    if content_type(&source).is_none() {
        return Err("That file is not a picture.".into());
    }
    let dest = preview_file(media, rel);
    write_preview(&source, &dest)?;
    open_file(&dest, "image/jpeg")
}

pub fn open_picture(rel: &str) -> Result<Opened, String> {
    if !in_library(rel) {
        return Err("That picture is not in Media.".into());
    }
    let full = file_in(&media_dir(), rel)?;
    let kind = content_type(&full).ok_or_else(|| "That file is not a picture.".to_string())?;
    let meta = fs::metadata(&full).map_err(|_| "That picture is not in Media.".to_string())?;
    if !meta.is_file() {
        return Err("That file is not a picture.".into());
    }
    let file = fs::File::open(&full).map_err(|_| "Keel could not open that picture.".to_string())?;
    Ok(Opened {
        file,
        len: meta.len(),
        kind,
    })
}

pub fn library(media: &Path) -> Vec<Picture> {
    let mut pictures = Vec::new();
    for folder in LIBRARY {
        if pictures.len() >= PICTURE_LIMIT {
            break;
        }
        for picture in list_in(&media.join(folder)) {
            if pictures.len() >= PICTURE_LIMIT {
                break;
            }
            pictures.push(Picture {
                name: picture.name,
                path: format!("{folder}/{}", picture.path),
            });
        }
    }
    pictures.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    pictures
}

fn in_library(rel: &str) -> bool {
    let rel = rel.trim().trim_matches(['/', '\\']);
    let mut parts = rel.split(['/', '\\']).filter(|part| !part.is_empty());
    let Some(folder) = parts.next() else {
        return false;
    };
    LIBRARY.contains(&folder) && parts.next().is_some()
}

pub fn list_in(root: &Path) -> Vec<Picture> {
    let mut pictures = Vec::new();
    if root.is_dir() {
        collect(root, root, 0, &mut pictures);
        pictures.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    }
    pictures
}

pub fn file_in(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim().trim_matches(['/', '\\']);
    if rel.is_empty() {
        return Err("That picture is not in Media.".into());
    }
    let mut full = root.to_path_buf();
    for part in rel.split(['/', '\\']) {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return Err("That picture is not in Media.".into());
        }
        full.push(part);
    }
    let root_real = fs::canonicalize(root).map_err(|_| "Pictures live in Media. None are there yet.".to_string())?;
    let full_real = fs::canonicalize(&full).map_err(|_| "That picture is not in Media.".to_string())?;
    if !full_real.starts_with(&root_real) {
        return Err("That picture is not in Media.".into());
    }
    Ok(full_real)
}

fn preview_file(media: &Path, rel: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(rel.as_bytes());
    let digest = hasher.finalize();
    let name = format!("{}.jpg", hex(&digest[..16]));
    media
        .parent()
        .unwrap_or(media)
        .join(".keel")
        .join("previews")
        .join(name)
}

fn fresh(cache: &Path, source: &Path) -> bool {
    let Ok(cache_meta) = fs::metadata(cache) else {
        return false;
    };
    let Ok(source_meta) = fs::metadata(source) else {
        return false;
    };
    if cache_meta.len() == 0 {
        return false;
    }
    match (cache_meta.modified(), source_meta.modified()) {
        (Ok(cache_time), Ok(source_time)) => cache_time >= source_time,
        _ => false,
    }
}

fn write_preview(source: &Path, dest: &Path) -> Result<(), String> {
    let _gate = PREVIEW_GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    if fresh(dest, source) {
        return Ok(());
    }
    let image = image::open(source).map_err(|_| "Keel could not make a small copy of that picture.".to_string())?;
    let small = image.thumbnail(480, 480);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| "Keel could not keep a small copy on the disk.".to_string())?;
    }
    let tmp = dest.with_extension("part");
    small
        .save_with_format(&tmp, image::ImageFormat::Jpeg)
        .map_err(|_| "Keel could not keep a small copy on the disk.".to_string())?;
    fs::rename(&tmp, dest).map_err(|_| "Keel could not keep a small copy on the disk.".to_string())?;
    Ok(())
}

fn open_file(path: &Path, kind: &'static str) -> Result<Opened, String> {
    let meta = fs::metadata(path).map_err(|_| "Keel could not open that picture.".to_string())?;
    let file = fs::File::open(path).map_err(|_| "Keel could not open that picture.".to_string())?;
    Ok(Opened {
        file,
        len: meta.len(),
        kind,
    })
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn media_dir() -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from("/srv/keel/files/Media")
    } else {
        let dir = std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into());
        PathBuf::from(dir).join("mock-files").join("Media")
    }
}

fn collect(root: &Path, dir: &Path, depth: u32, out: &mut Vec<Picture>) {
    if depth > WALK_DEPTH || out.len() >= PICTURE_LIMIT {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        if out.len() >= PICTURE_LIMIT {
            return;
        }
        let path = item.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect(root, &path, depth + 1, out);
            continue;
        }
        if !meta.is_file() || content_type(&path).is_none() {
            continue;
        }
        let Some(rel) = path.strip_prefix(root).ok().and_then(|rel| rel.to_str()) else {
            continue;
        };
        let rel = rel.replace('\\', "/");
        if rel.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("picture")
            .to_string();
        out.push(Picture { name, path: rel });
    }
}

fn content_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("png") => Some("image/png"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{file_in, in_library, library, list_in, open_small_in};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn the_light_page_lists_pictures_already_in_media() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("keel-photos-{stamp}"));
        let nested = root.join("Holiday");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("a.jpg"), b"jpeg").unwrap();
        fs::write(root.join("note.txt"), b"not a picture").unwrap();
        fs::write(root.join("b.PNG"), b"png").unwrap();
        let listed = list_in(&root);
        let paths: Vec<&str> = listed.iter().map(|picture| picture.path.as_str()).collect();
        assert!(paths.contains(&"Holiday/a.jpg"));
        assert!(paths.contains(&"b.PNG"));
        assert!(!paths.iter().any(|path| path.ends_with("note.txt")));
        assert!(file_in(&root, "Holiday/a.jpg").is_ok());
        assert!(file_in(&root, "../secret.jpg").is_err());
        assert!(file_in(&root, "note.txt").unwrap().ends_with("note.txt"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_page_shows_the_pictures_folder_beside_photos() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let media = std::env::temp_dir().join(format!("keel-media-{stamp}"));
        fs::create_dir_all(media.join("Photos")).unwrap();
        fs::create_dir_all(media.join("Pictures").join("Holiday")).unwrap();
        fs::create_dir_all(media.join("Audiobooks")).unwrap();
        fs::write(media.join("Pictures").join("Holiday").join("a.jpg"), b"jpeg").unwrap();
        fs::write(media.join("Audiobooks").join("cover.jpg"), b"jpeg").unwrap();
        let listed = library(&media);
        let paths: Vec<&str> = listed.iter().map(|picture| picture.path.as_str()).collect();
        assert_eq!(paths, vec!["Pictures/Holiday/a.jpg"]);
        assert!(in_library("Pictures/Holiday/a.jpg"));
        assert!(!in_library("Audiobooks/cover.jpg"));
        assert!(!in_library("../secret.jpg"));
        let _ = fs::remove_dir_all(&media);
    }

    #[test]
    fn a_small_copy_is_kept_on_the_files_disk() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-preview-{stamp}"));
        let media = disk.join("Media");
        fs::create_dir_all(media.join("Pictures")).unwrap();
        image::RgbImage::from_pixel(32, 32, image::Rgb([9, 8, 7]))
            .save(media.join("Pictures").join("a.png"))
            .unwrap();
        let opened = open_small_in(&media, "Pictures/a.png").unwrap();
        assert_eq!(opened.kind, "image/jpeg");
        assert!(opened.len > 0);
        let saved = fs::read_dir(disk.join(".keel").join("previews")).unwrap().count();
        assert_eq!(saved, 1);
        assert!(!media.join(".keel").exists());
        let again = open_small_in(&media, "Pictures/a.png").unwrap();
        assert_eq!(again.kind, "image/jpeg");
        let _ = fs::remove_dir_all(&disk);
    }
}
