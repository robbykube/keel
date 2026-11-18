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
const CATEGORY_LIMIT: usize = 8000;
const WALK_DEPTH: u32 = 6;
const LIBRARY: &[&str] = &["Photos", "Pictures"];
const LOOSE: &str = "loose";

#[derive(Serialize)]
pub struct Picture {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct Category {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct Album {
    pub categories: Vec<Category>,
    pub pictures: Vec<Picture>,
    pub message: String,
}

pub struct Opened {
    pub file: fs::File,
    pub len: u64,
    pub kind: &'static str,
}

pub fn album() -> Album {
    let categories = categories_in(&media_dir());
    let message = if categories.is_empty() {
        "Pictures live in Media. None are there yet.".into()
    } else {
        "Choose a category. The pictures stay on the disk.".into()
    };
    Album {
        categories,
        pictures: Vec::new(),
        message,
    }
}

pub fn open_category(category: &str) -> Album {
    match pictures_for(&media_dir(), category) {
        Ok(pictures) if pictures.is_empty() => Album {
            categories: Vec::new(),
            pictures,
            message: "No pictures in that category yet.".into(),
        },
        Ok(pictures) => Album {
            categories: Vec::new(),
            pictures,
            message: "Pictures already in Media. They stay on the disk. Small copies stay on the disk with them.".into(),
        },
        Err(message) => Album {
            categories: Vec::new(),
            pictures: Vec::new(),
            message,
        },
    }
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

pub fn categories_in(media: &Path) -> Vec<Category> {
    let mut categories = Vec::new();
    let mut loose = false;
    for folder in LIBRARY {
        let root = media.join(folder);
        if !root.is_dir() {
            continue;
        }
        let Ok(read) = fs::read_dir(&root) else {
            continue;
        };
        for item in read.flatten() {
            let path = item.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with('.') || !part_ok(name) {
                continue;
            }
            if meta.is_dir() {
                if has_picture(&path, 1) {
                    categories.push(Category {
                        name: name.to_string(),
                        path: format!("{folder}/{name}"),
                    });
                }
            } else if meta.is_file() && content_type(&path).is_some() {
                loose = true;
            }
        }
    }
    let names: Vec<String> = categories.iter().map(|category| category.name.clone()).collect();
    for category in &mut categories {
        let copies = names.iter().filter(|name| *name == &category.name).count();
        if copies > 1 {
            let parent = category.path.split('/').next().unwrap_or("");
            category.name = format!("{} in {parent}", category.name);
        }
    }
    categories.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    if loose {
        categories.push(Category {
            name: "Pictures not in a folder".into(),
            path: LOOSE.into(),
        });
    }
    categories
}

pub fn pictures_for(media: &Path, category: &str) -> Result<Vec<Picture>, String> {
    if category == LOOSE {
        return Ok(loose_pictures(media));
    }
    let rel = category.trim().trim_matches(['/', '\\']);
    let parts: Vec<&str> = rel.split(['/', '\\']).filter(|part| !part.is_empty()).collect();
    if parts.len() != 2 || !LIBRARY.contains(&parts[0]) || !part_ok(parts[1]) {
        return Err("That category is not in Media.".into());
    }
    let dir = media.join(parts[0]).join(parts[1]);
    if !dir.is_dir() {
        return Err("That category is not in Media.".into());
    }
    let mut pictures = list_capped(&dir, CATEGORY_LIMIT);
    for picture in &mut pictures {
        picture.path = format!("{rel}/{}", picture.path);
    }
    Ok(pictures)
}

fn loose_pictures(media: &Path) -> Vec<Picture> {
    let mut pictures = Vec::new();
    for folder in LIBRARY {
        let root = media.join(folder);
        let Ok(read) = fs::read_dir(&root) else {
            continue;
        };
        for item in read.flatten() {
            if pictures.len() >= CATEGORY_LIMIT {
                break;
            }
            let path = item.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() || !meta.is_file() || content_type(&path).is_none() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !part_ok(name) {
                continue;
            }
            pictures.push(Picture {
                name: name.to_string(),
                path: format!("{folder}/{name}"),
            });
        }
    }
    pictures.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    pictures
}

fn has_picture(dir: &Path, depth: u32) -> bool {
    if depth > WALK_DEPTH {
        return false;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return false;
    };
    for item in read.flatten() {
        let path = item.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() && has_picture(&path, depth + 1) {
            return true;
        }
        if meta.is_file() && content_type(&path).is_some() {
            return true;
        }
    }
    false
}

fn part_ok(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && !part.contains(':')
        && !part.contains('/')
        && !part.contains('\\')
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
    list_capped(root, PICTURE_LIMIT)
}

fn list_capped(root: &Path, limit: usize) -> Vec<Picture> {
    let mut pictures = Vec::new();
    if root.is_dir() {
        collect(root, root, 0, limit, &mut pictures);
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

fn collect(root: &Path, dir: &Path, depth: u32, limit: usize, out: &mut Vec<Picture>) {
    if depth > WALK_DEPTH || out.len() >= limit {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        if out.len() >= limit {
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
            collect(root, &path, depth + 1, limit, out);
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
    use super::{categories_in, file_in, in_library, library, list_in, open_small_in, pictures_for};
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

    #[test]
    fn categories_are_the_folders_already_there() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let media = std::env::temp_dir().join(format!("keel-categories-{stamp}"));
        fs::create_dir_all(media.join("Pictures").join("Holiday")).unwrap();
        fs::create_dir_all(media.join("Pictures").join("Empty")).unwrap();
        fs::create_dir_all(media.join("Photos")).unwrap();
        fs::create_dir_all(media.join("Audiobooks")).unwrap();
        fs::write(media.join("Pictures").join("Holiday").join("a.jpg"), b"jpeg").unwrap();
        fs::write(media.join("Pictures").join("loose.jpg"), b"jpeg").unwrap();
        fs::write(media.join("Pictures").join("Empty").join("note.txt"), b"text").unwrap();
        fs::write(media.join("Audiobooks").join("cover.jpg"), b"jpeg").unwrap();
        let categories = categories_in(&media);
        let names: Vec<&str> = categories.iter().map(|category| category.name.as_str()).collect();
        assert_eq!(names, vec!["Holiday", "Pictures not in a folder"]);
        let holiday = pictures_for(&media, "Pictures/Holiday").unwrap();
        assert_eq!(holiday[0].path, "Pictures/Holiday/a.jpg");
        let loose = pictures_for(&media, "loose").unwrap();
        assert_eq!(loose[0].path, "Pictures/loose.jpg");
        assert!(pictures_for(&media, "../secret").is_err());
        assert!(pictures_for(&media, "Audiobooks/cover").is_err());
        let _ = fs::remove_dir_all(&media);
    }
}
