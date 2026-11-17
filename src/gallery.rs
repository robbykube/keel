//! The light Photos page. It shows picture files already in Media.
//! The library may be named Photos or Pictures. Nothing is downloaded,
//! and the pictures stay on the disk.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

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
        "Pictures already in Media. They stay on the disk.".into()
    };
    Album { pictures, message }
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
    use super::{file_in, in_library, library, list_in};
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
}
