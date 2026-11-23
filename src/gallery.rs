//! The light Photos page. It shows the pictures and videos already in Media.
//! The library may be named Photos or Pictures. Nothing is downloaded,
//! and the pictures stay on the disk.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::picture_dates::{self, Wanted};

/// The longest side of the copy in the grid of pictures.
const GRID_SIDE: u32 = 240;
/// The longest side of the copy on a folder tile and while a picture opens.
const SMALL_SIDE: u32 = 480;
/// The longest side of the copy shown when a picture is open. Big enough for
/// a full screen, small enough that a Pi does not hold whole camera pictures.
const SCREEN_SIDE: u32 = 1920;

/// The copies on the files disk stay under a share of that disk: 2%, and
/// never more than 4 GB. The least recently opened go first; any copy is made
/// again the next time its picture is opened.
const PREVIEW_MAX: u64 = 4 * 1024 * 1024 * 1024;
const PREVIEW_MIN: u64 = 512 * 1024 * 1024;
const PREVIEW_SHARE: u64 = 50;
/// Clean after this many new copies, as well as once a day.
const TIDY_EVERY: usize = 500;
/// Walking the library stops here; past it, Keel cannot tell which copies are
/// left over, so it keeps them.
const TIDY_WALK_LIMIT: usize = 250_000;
const DAY: Duration = Duration::from_secs(24 * 60 * 60);
/// How long to wait before trying again to add a helper program.
const HELPER_RETRY: Duration = Duration::from_secs(60 * 60);

static WRITTEN: AtomicUsize = AtomicUsize::new(0);
static TIDYING: AtomicBool = AtomicBool::new(false);
static CONVERTED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Grid,
    Small,
    Screen,
}

/// The copies of one picture, kept beside each other.
#[derive(Clone)]
struct Copies {
    grid: PathBuf,
    small: PathBuf,
    screen: PathBuf,
}

impl Copies {
    fn of(&self, size: Size) -> &Path {
        match size {
            Size::Grid => &self.grid,
            Size::Small => &self.small,
            Size::Screen => &self.screen,
        }
    }

    /// The grid and small copies are made together, from the picture read at
    /// a small size; the screen copy is made when a picture is opened.
    fn ready(&self, size: Size, source: &Path) -> bool {
        match size {
            Size::Grid | Size::Small => fresh(&self.grid, source) && fresh(&self.small, source),
            Size::Screen => fresh(&self.screen, source),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Picture,
    Video,
}

const PICTURE_LIMIT: usize = 400;
const CATEGORY_LIMIT: usize = 8000;
const WALK_DEPTH: u32 = 6;
const LIBRARY: &[&str] = &["Photos", "Pictures"];
const LOOSE: &str = "loose";

#[derive(Serialize, Clone, Debug)]
pub struct Picture {
    pub name: String,
    pub path: String,
    /// A video plays as it is; it has no copies.
    #[serde(skip_serializing_if = "is_false")]
    pub video: bool,
    /// When it was taken, in seconds. The camera's own clock when the picture
    /// says, otherwise the date on the file.
    pub taken: i64,
    /// Changes when the file changes, so a browser may keep its copies.
    pub v: String,
    /// The picture's average colour, shown on its tile while it loads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip)]
    modified: i64,
    #[serde(skip)]
    len: u64,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Serialize)]
pub struct Category {
    pub name: String,
    pub path: String,
    /// One picture from the folder, shown on its tile. Empty when the folder
    /// holds only videos.
    pub cover: String,
    pub cover_v: String,
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
    match pictures_sorted(&media_dir(), category) {
        Ok((pictures, _)) if pictures.is_empty() => Album {
            categories: Vec::new(),
            pictures,
            message: "No pictures in that category yet.".into(),
        },
        Ok((pictures, complete)) => Album {
            categories: Vec::new(),
            pictures,
            message: if complete {
                "Newest first. The pictures stay on the disk.".into()
            } else {
                "Newest first. Keel is still reading when some were taken, so the order may change.".into()
            },
        },
        Err(message) => Album {
            categories: Vec::new(),
            pictures: Vec::new(),
            message,
        },
    }
}

/// What it takes to answer for a copy of one picture.
pub struct CopyPlan {
    source: PathBuf,
    copies: Copies,
    legacy: PathBuf,
    /// Where the picture's colour is kept: the library and the picture's place
    /// in it, with the file's modified time and size. None outside Photos.
    colour: Option<(PathBuf, String, i64, u64)>,
    size: Size,
    /// The copies are there and newer than the picture.
    pub ready: bool,
}

pub fn plan_copy(rel: &str, size: Size) -> Result<CopyPlan, String> {
    plan_copy_in(&media_dir(), rel, size)
}

fn plan_copy_in(media: &Path, rel: &str, size: Size) -> Result<CopyPlan, String> {
    if !in_library(rel) {
        return Err("That picture is not in Media.".into());
    }
    let source = file_in(media, rel)?;
    if kind_of(&source).is_none() {
        return Err("That file is not a picture.".into());
    }
    let copies = Copies {
        grid: preview_file(media, rel, Size::Grid),
        small: preview_file(media, rel, Size::Small),
        screen: preview_file(media, rel, Size::Screen),
    };
    let ready = copies.ready(size, &source);
    let colour = fs::metadata(&source)
        .ok()
        .map(|meta| (media.to_path_buf(), rel.trim_matches(['/', '\\']).to_string(), seconds(&meta), meta.len()));
    Ok(CopyPlan {
        source,
        copies,
        colour,
        legacy: legacy_preview_file(media, rel),
        size,
        ready,
    })
}

/// Asks for the copies. They are made a few at a time, newest request first,
/// at low priority; a request nobody waits for any more is dropped. The answer
/// says whether copies were written.
pub fn queue_copies(plan: &CopyPlan) -> tokio::sync::oneshot::Receiver<Result<bool, String>> {
    let (reply, answer) = tokio::sync::oneshot::channel();
    let queue = queue();
    let mut jobs = queue.jobs.lock().unwrap_or_else(|poison| poison.into_inner());
    jobs.push(Job {
        source: plan.source.clone(),
        copies: plan.copies.clone(),
        size: plan.size,
        open: plan.size == Size::Screen,
        reply,
    });
    drop(jobs);
    queue.wake.notify_one();
    answer
}

pub fn finish_copy(plan: &CopyPlan, written: bool) -> Result<Opened, String> {
    if written {
        let _ = fs::remove_file(&plan.legacy);
        if (WRITTEN.fetch_add(1, Ordering::Relaxed) + 1) % TIDY_EVERY == 0 {
            std::thread::spawn(tidy_previews);
        }
    }
    let served = plan.copies.of(plan.size);
    mark_opened(served);
    if plan.size != Size::Screen {
        remember_colour(plan);
    }
    open_file(served, "image/jpeg")
}

/// Notes the picture's colour from its grid copy, once per version of the
/// file. Copies made before colours were kept get theirs this way too.
fn remember_colour(plan: &CopyPlan) {
    let Some((media, rel, modified, len)) = &plan.colour else {
        return;
    };
    if picture_dates::colours_for(media, &[(rel.as_str(), *modified, *len)])[0].is_some() {
        return;
    }
    if let Ok(grid) = image::open(&plan.copies.grid) {
        let colour = picture_dates::average_colour(&grid.to_rgb8());
        picture_dates::set_colour(media, rel, *modified, *len, colour);
    }
}

/// A copy of a picture anywhere in Files, outside the Photos library. These
/// are kept in Keel's own folder, not beside the picture, and take a small
/// share of that disk.
pub fn plan_copy_file(source: PathBuf, place: &str, size: Size) -> Result<CopyPlan, String> {
    if kind_of(&source).is_none() {
        return Err("That file is not a picture.".into());
    }
    let dir = files_previews_dir();
    let name = preview_name(&format!("files:{place}"));
    let copies = Copies {
        grid: dir.join(format!("{name}-grid.jpg")),
        small: dir.join(format!("{name}-small.jpg")),
        screen: dir.join(format!("{name}-screen.jpg")),
    };
    let ready = copies.ready(size, &source);
    Ok(CopyPlan {
        legacy: dir.join(format!("{name}-none.jpg")),
        source,
        copies,
        colour: None,
        size,
        ready,
    })
}

fn files_previews_dir() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join("previews")
}

/// A copy of the picture kept on the files disk, made once. The small copy and
/// the screen copy come from one read of the picture. This waits for it.
#[cfg(test)]
pub fn open_sized_in(media: &Path, rel: &str, size: Size) -> Result<Opened, String> {
    let plan = plan_copy_in(media, rel, size)?;
    let written = if plan.ready {
        false
    } else {
        queue_copies(&plan)
            .blocking_recv()
            .unwrap_or_else(|_| Err("Keel could not make a small copy of that picture.".into()))?
    };
    finish_copy(&plan, written)
}

/// The picture or video itself, for a download or for a video to play.
pub fn original(rel: &str) -> Result<PathBuf, String> {
    if !in_library(rel) {
        return Err("That picture is not in Media.".into());
    }
    let full = file_in(&media_dir(), rel)?;
    if kind_of(&full).is_none() || !full.is_file() {
        return Err("That file is not a picture.".into());
    }
    Ok(full)
}

/// The picture or video itself, opened so the browser can save it.
pub struct SavedFile {
    pub file: std::fs::File,
    pub name: String,
    pub len: u64,
}

pub fn open_original(rel: &str) -> Result<SavedFile, String> {
    let full = original(rel)?;
    let name = full
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.contains(['/', '\\', '"']) && !name.chars().any(char::is_control))
        .ok_or_else(|| "That file name cannot be downloaded.".to_string())?
        .to_string();
    let meta = std::fs::metadata(&full).map_err(|_| "That picture is not there.".to_string())?;
    let file = std::fs::File::open(&full).map_err(|_| "Keel could not download that picture.".to_string())?;
    Ok(SavedFile { file, name, len: meta.len() })
}

struct Job {
    source: PathBuf,
    copies: Copies,
    size: Size,
    /// For a picture someone has open, which goes before the grid.
    open: bool,
    reply: tokio::sync::oneshot::Sender<Result<bool, String>>,
}

struct Queue {
    jobs: Mutex<Vec<Job>>,
    wake: Condvar,
}

fn queue() -> &'static Queue {
    static QUEUE: OnceLock<Queue> = OnceLock::new();
    QUEUE.get_or_init(|| {
        // Two at a time on a computer with four cores or more, so the screen
        // and the file sharing keep the rest.
        let cores = std::thread::available_parallelism().map_or(1, |cores| cores.get());
        let workers = if cores >= 4 { 2 } else { 1 };
        for _ in 0..workers {
            std::thread::Builder::new()
                .name("keel-copies".into())
                .spawn(copy_worker)
                .expect("Keel could not start making picture copies");
        }
        Queue {
            jobs: Mutex::new(Vec::new()),
            wake: Condvar::new(),
        }
    })
}

/// The newest request someone still waits for, with an open picture before
/// the grid's small copies. Requests nobody waits for any more are dropped:
/// their page moved on.
fn take_next(jobs: &mut Vec<Job>) -> Option<Job> {
    jobs.retain(|job| !job.reply.is_closed());
    match jobs.iter().rposition(|job| job.open) {
        Some(index) => Some(jobs.remove(index)),
        None => jobs.pop(),
    }
}

fn copy_worker() {
    lower_priority();
    let queue = queue();
    loop {
        let job = {
            let mut jobs = queue.jobs.lock().unwrap_or_else(|poison| poison.into_inner());
            loop {
                if let Some(job) = take_next(&mut jobs) {
                    break job;
                }
                jobs = queue.wake.wait(jobs).unwrap_or_else(|poison| poison.into_inner());
            }
        };
        let result = std::panic::catch_unwind(|| write_previews(&job.source, &job.copies, job.size))
            .unwrap_or_else(|_| Err("Keel could not make a small copy of that picture.".into()));
        let _ = job.reply.send(result);
    }
}

/// Work that can wait gives way to the screen and the file sharing.
pub(crate) fn lower_priority() {
    #[cfg(target_os = "linux")]
    // SAFETY: setpriority with who = 0 changes only the calling thread.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS as _, 0, 10);
    }
}

/// Remembers when a copy was last opened, at most once a day, so cleaning
/// keeps the ones in use. The copy only gets newer, so it stays fresh.
fn mark_opened(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    let old = meta
        .modified()
        .ok()
        .and_then(|time| SystemTime::now().duration_since(time).ok())
        .map_or(false, |age| age > DAY);
    if old {
        if let Ok(file) = fs::File::options().write(true).open(path) {
            let _ = file.set_modified(SystemTime::now());
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tidied {
    pub removed: usize,
    pub bytes: u64,
}

/// Removes copies whose picture is gone, half-written copies, and then the
/// least recently opened copies while they take more than their share.
pub fn tidy_previews() {
    if TIDYING.swap(true, Ordering::AcqRel) {
        return;
    }
    let media = media_dir();
    let (limit, target) = preview_limits(&media);
    let mut tidied = tidy_in(&media, limit, target);
    // Copies of pictures elsewhere in Files are on Keel's own disk; they get a
    // quarter of the share the Photos copies get.
    let files = files_previews_dir();
    let (limit, target) = match disk_space(files.parent().unwrap_or(&files)) {
        Some((total, free)) => limits_for(total, free),
        None => (PREVIEW_MIN, PREVIEW_MIN / 10 * 8),
    };
    let more = tidy_dir(&files, None, limit / 4, target / 4);
    tidied.removed += more.removed;
    tidied.bytes += more.bytes;
    if tidied.removed > 0 {
        eprintln!(
            "keel: removed {} picture copies ({} MB)",
            tidied.removed,
            tidied.bytes / (1024 * 1024)
        );
    }
    TIDYING.store(false, Ordering::Release);
}

fn preview_limits(media: &Path) -> (u64, u64) {
    let disk = media.parent().unwrap_or(media);
    match disk_space(disk) {
        Some((total, free)) => limits_for(total, free),
        None => (PREVIEW_MAX, PREVIEW_MAX / 10 * 8),
    }
}

/// The most the copies may take, and where cleaning stops, for a disk of this
/// size. A nearly full disk keeps far fewer.
fn limits_for(total: u64, free: u64) -> (u64, u64) {
    let limit = (total / PREVIEW_SHARE).clamp(PREVIEW_MIN, PREVIEW_MAX);
    if free < total / 20 {
        (limit / 4, limit / 8)
    } else {
        (limit, limit / 10 * 8)
    }
}

#[cfg(unix)]
fn disk_space(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs fills the struct it is given; the path is a valid C string.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = stat.f_frsize as u64;
    Some((stat.f_blocks as u64 * block, stat.f_bavail as u64 * block))
}

#[cfg(not(unix))]
fn disk_space(_path: &Path) -> Option<(u64, u64)> {
    None
}

pub fn tidy_in(media: &Path, limit: u64, target: u64) -> Tidied {
    let known = known_media(media);
    if let Some((_, rels)) = &known {
        picture_dates::prune(media, rels);
        picture_dates::prune_colours(media, rels);
    }
    tidy_dir(&previews_dir(media), known.as_ref().map(|(hashes, _)| hashes), limit, target)
}

/// Cleans one folder of copies. With `known`, copies of pictures not in it
/// are left over and go; without it, only the oldest go, past the limit.
fn tidy_dir(dir: &Path, known: Option<&HashSet<String>>, limit: u64, target: u64) -> Tidied {
    let mut tidied = Tidied::default();
    let Ok(read) = fs::read_dir(dir) else {
        return tidied;
    };
    let now = SystemTime::now();
    let mut kept = Vec::new();
    let mut total = 0u64;
    for item in read.flatten() {
        let path = item.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let modified = meta.modified().unwrap_or(now);
        let age = now.duration_since(modified).unwrap_or_default();
        let gone = if let Some(stem) = name.strip_suffix(".part") {
            age > DAY || stem.is_empty()
        } else if let Some(stem) = name.strip_suffix(".jpg") {
            let picture = stem
                .strip_suffix("-small")
                .or_else(|| stem.strip_suffix("-grid"))
                .or_else(|| stem.strip_suffix("-screen"))
                .unwrap_or(stem);
            known.map_or(false, |hashes| !hashes.contains(picture))
        } else {
            false
        };
        if gone {
            if fs::remove_file(&path).is_ok() {
                tidied.removed += 1;
                tidied.bytes += meta.len();
            }
            continue;
        }
        if name.ends_with(".jpg") {
            total += meta.len();
            kept.push((modified, meta.len(), path));
        }
    }
    if total > limit {
        kept.sort_by_key(|(modified, _, _)| *modified);
        for (_, len, path) in kept {
            if total <= target {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
                tidied.removed += 1;
                tidied.bytes += len;
            }
        }
    }
    tidied
}

/// The copy names every picture in the library would have, and the pictures'
/// paths. None when the library is not there, or too large to walk, so
/// nothing counts as left over.
fn known_media(media: &Path) -> Option<(HashSet<String>, HashSet<String>)> {
    let mut found = false;
    let mut hashes = HashSet::new();
    let mut rels = HashSet::new();
    for folder in LIBRARY {
        let root = media.join(folder);
        if !root.is_dir() {
            continue;
        }
        found = true;
        let pictures = list_capped(&root, TIDY_WALK_LIMIT);
        if pictures.len() >= TIDY_WALK_LIMIT {
            return None;
        }
        for picture in pictures {
            let rel = format!("{folder}/{}", picture.path);
            hashes.insert(preview_name(&rel));
            rels.insert(rel);
        }
    }
    found.then_some((hashes, rels))
}

pub fn library(media: &Path) -> Vec<Picture> {
    let mut pictures = Vec::new();
    for folder in LIBRARY {
        if pictures.len() >= PICTURE_LIMIT {
            break;
        }
        for mut picture in list_in(&media.join(folder)) {
            if pictures.len() >= PICTURE_LIMIT {
                break;
            }
            picture.path = format!("{folder}/{}", picture.path);
            pictures.push(picture);
        }
    }
    pictures.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    pictures
}

pub fn categories_in(media: &Path) -> Vec<Category> {
    let mut categories = Vec::new();
    let mut loose_any = false;
    let mut loose_cover: Option<String> = None;
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
                if let Some((first, kind)) = first_media(&path, 1) {
                    let cover = if kind == Kind::Picture {
                        format!("{folder}/{name}/{first}")
                    } else {
                        String::new()
                    };
                    categories.push(Category {
                        name: name.to_string(),
                        path: format!("{folder}/{name}"),
                        cover_v: version_of(media, &cover),
                        cover,
                    });
                }
            } else if meta.is_file() {
                match kind_of(&path) {
                    Some(Kind::Picture) => {
                        loose_any = true;
                        let candidate = format!("{folder}/{name}");
                        if loose_cover
                            .as_ref()
                            .map_or(true, |seen| candidate.to_ascii_lowercase() < seen.to_ascii_lowercase())
                        {
                            loose_cover = Some(candidate);
                        }
                    }
                    Some(Kind::Video) => loose_any = true,
                    None => {}
                }
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
    if loose_any {
        let cover = loose_cover.unwrap_or_default();
        categories.push(Category {
            name: "Pictures not in a folder".into(),
            path: LOOSE.into(),
            cover_v: version_of(media, &cover),
            cover,
        });
    }
    categories
}

fn version_of(media: &Path, rel: &str) -> String {
    if rel.is_empty() {
        return String::new();
    }
    fs::metadata(media.join(rel))
        .map(|meta| version(seconds(&meta), meta.len()))
        .unwrap_or_default()
}

fn version(modified: i64, len: u64) -> String {
    format!("{modified:x}{len:x}")
}

fn seconds(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs() as i64)
}

#[cfg(test)]
pub fn pictures_for(media: &Path, category: &str) -> Result<Vec<Picture>, String> {
    pictures_sorted(media, category).map(|(pictures, _)| pictures)
}

/// The pictures in a category, newest first. The answer also says whether
/// every picture's taken date was known yet.
fn pictures_sorted(media: &Path, category: &str) -> Result<(Vec<Picture>, bool), String> {
    let mut pictures = if category == LOOSE {
        loose_pictures(media)
    } else {
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
        pictures
    };
    let wanted = pictures
        .iter()
        .map(|picture| Wanted {
            rel: picture.path.clone(),
            file: media.join(&picture.path),
            modified: picture.modified,
            len: picture.len,
        })
        .collect();
    let (found, complete) = picture_dates::look_up(media, wanted);
    for (picture, taken) in pictures.iter_mut().zip(found) {
        if let Some(taken) = taken {
            picture.taken = taken;
        }
    }
    let asked: Vec<(&str, i64, u64)> = pictures.iter().map(|p| (p.path.as_str(), p.modified, p.len)).collect();
    let colours = picture_dates::colours_for(media, &asked);
    for (picture, colour) in pictures.iter_mut().zip(colours) {
        picture.color = colour.map(|colour| format!("#{colour:06x}"));
    }
    pictures.sort_by(|a, b| {
        b.taken
            .cmp(&a.taken)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    Ok((pictures, complete))
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
            if meta.file_type().is_symlink() || !meta.is_file() {
                continue;
            }
            let Some(kind) = kind_of(&path) else {
                continue;
            };
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !part_ok(name) {
                continue;
            }
            pictures.push(entry(name, format!("{folder}/{name}"), kind, &meta));
        }
    }
    pictures
}

fn entry(name: &str, path: String, kind: Kind, meta: &fs::Metadata) -> Picture {
    let modified = seconds(meta);
    Picture {
        name: name.to_string(),
        path,
        video: kind == Kind::Video,
        taken: modified,
        v: version(modified, meta.len()),
        color: None,
        modified,
        len: meta.len(),
    }
}

/// The first picture in a folder, by name, as a path inside that folder, or
/// the first video when there is no picture. Files directly in the folder come
/// before ones in folders below it, so the tile shows what the folder is
/// mostly about.
fn first_media(dir: &Path, depth: u32) -> Option<(String, Kind)> {
    if depth > WALK_DEPTH {
        return None;
    }
    let read = fs::read_dir(dir).ok()?;
    let mut pictures = Vec::new();
    let mut videos = Vec::new();
    let mut dirs = Vec::new();
    for item in read.flatten() {
        let path = item.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()).map(str::to_string) else {
            continue;
        };
        if name.starts_with('.') || !part_ok(&name) {
            continue;
        }
        if meta.is_file() {
            match kind_of(&path) {
                Some(Kind::Picture) => pictures.push(name),
                Some(Kind::Video) => videos.push(name),
                None => {}
            }
        } else if meta.is_dir() {
            dirs.push(name);
        }
    }
    pictures.sort_by_key(|name| name.to_ascii_lowercase());
    if let Some(first) = pictures.into_iter().next() {
        return Some((first, Kind::Picture));
    }
    dirs.sort_by_key(|name| name.to_ascii_lowercase());
    let mut video_below = None;
    for name in dirs {
        match first_media(&dir.join(&name), depth + 1) {
            Some((inner, Kind::Picture)) => return Some((format!("{name}/{inner}"), Kind::Picture)),
            Some((inner, Kind::Video)) if video_below.is_none() => {
                video_below = Some((format!("{name}/{inner}"), Kind::Video));
            }
            _ => {}
        }
    }
    videos.sort_by_key(|name| name.to_ascii_lowercase());
    videos
        .into_iter()
        .next()
        .map(|first| (first, Kind::Video))
        .or(video_below)
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

fn preview_name(rel: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rel.as_bytes());
    hex(&hasher.finalize()[..16])
}

fn previews_dir(media: &Path) -> PathBuf {
    media.parent().unwrap_or(media).join(".keel").join("previews")
}

fn preview_file(media: &Path, rel: &str, size: Size) -> PathBuf {
    let kind = match size {
        Size::Grid => "grid",
        Size::Small => "small",
        Size::Screen => "screen",
    };
    previews_dir(media).join(format!("{}-{kind}.jpg", preview_name(rel)))
}

/// Small copies before v0.1.55 did not turn phone pictures upright.
fn legacy_preview_file(media: &Path, rel: &str) -> PathBuf {
    previews_dir(media).join(format!("{}.jpg", preview_name(rel)))
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

/// Makes the copies a request needs when they are missing or older than the
/// picture. True when it wrote any. A JPEG is read only as large as the copy
/// needs, which is most of the work saved. Each copy made is written to the
/// log with how long it took.
fn write_previews(source: &Path, copies: &Copies, size: Size) -> Result<bool, String> {
    if copies.ready(size, source) {
        return Ok(false);
    }
    let started = Instant::now();
    let thumbs = size != Size::Screen;
    // A grid copy is all that is missing: make it from the small copy.
    if thumbs && fresh(&copies.small, source) && !fresh(&copies.grid, source) {
        if let Ok(small) = image::open(&copies.small) {
            save_jpeg(&small.thumbnail(GRID_SIDE, GRID_SIDE), &copies.grid, 80)?;
            log_copy(source, "grid", started, started.elapsed(), (small.width(), small.height()));
            return Ok(true);
        }
    }
    let want = if thumbs { SMALL_SIDE } else { SCREEN_SIDE };
    let (picture, whole) = read_for(source, want)?;
    let read = started.elapsed();
    let read_size = (picture.width(), picture.height());
    let picture = image::DynamicImage::ImageRgb8(picture.to_rgb8());
    let fits = |side: u32| {
        if picture.width() > side || picture.height() > side {
            picture.thumbnail(side, side)
        } else {
            picture.clone()
        }
    };
    // Whatever this read can make well is kept, so it need not be read again.
    let can_screen = !thumbs || whole;
    if can_screen && !fresh(&copies.screen, source) {
        save_jpeg(&fits(SCREEN_SIDE), &copies.screen, 85)?;
    }
    if !fresh(&copies.small, source) || !fresh(&copies.grid, source) {
        let small = fits(SMALL_SIDE);
        save_jpeg(&small.thumbnail(GRID_SIDE, GRID_SIDE), &copies.grid, 80)?;
        save_jpeg(&small, &copies.small, 80)?;
    }
    log_copy(source, if thumbs { "small" } else { "screen" }, started, read, read_size);
    Ok(true)
}

fn log_copy(source: &Path, made: &str, started: Instant, read: Duration, read_size: (u32, u32)) {
    let name = source.file_name().and_then(|name| name.to_str()).unwrap_or("picture");
    let total = started.elapsed();
    eprintln!(
        "keel: picture copy {made} {name}: {:.2} s, reading {:.2} s at {}x{}, saving {:.2} s",
        total.as_secs_f64(),
        read.as_secs_f64(),
        read_size.0,
        read_size.1,
        total.saturating_sub(read).as_secs_f64(),
    );
}

/// The picture, upright, and whether it was read whole rather than at a
/// smaller size.
fn read_for(source: &Path, want: u32) -> Result<(image::DynamicImage, bool), String> {
    if matches!(extension(source).as_deref(), Some("jpg" | "jpeg")) {
        if let Some(picture) = read_jpeg_at(source, want) {
            return Ok(picture);
        }
    }
    read_upright(source).map(|picture| (picture, true))
}

/// Reads a JPEG at 1/2, 1/4 or 1/8 of its size, the smallest that is still
/// at least `want` on its long side. None when this reader cannot, and the
/// whole picture is read instead.
fn read_jpeg_at(source: &Path, want: u32) -> Option<(image::DynamicImage, bool)> {
    let file = fs::File::open(source).ok()?;
    let mut decoder = jpeg_decoder::Decoder::new(std::io::BufReader::new(file));
    decoder.read_info().ok()?;
    let info = decoder.info()?;
    let (width, height) = (info.width, info.height);
    let longest = width.max(height) as u32;
    let (mut w, mut h) = (width, height);
    if longest > want {
        let part = want as f64 / longest as f64;
        let ask = |side: u16| ((side as f64 * part).ceil() as u16).max(1);
        (w, h) = decoder.scale(ask(width), ask(height)).ok()?;
    }
    let pixels = decoder.decode().ok()?;
    let format = decoder.info()?.pixel_format;
    let mut picture = match format {
        jpeg_decoder::PixelFormat::RGB24 => {
            image::DynamicImage::ImageRgb8(image::RgbImage::from_raw(w as u32, h as u32, pixels)?)
        }
        jpeg_decoder::PixelFormat::L8 => {
            image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w as u32, h as u32, pixels)?)
        }
        _ => return None,
    };
    let turn = decoder
        .exif_data()
        .and_then(picture_dates::orientation_in_tiff)
        .and_then(image::metadata::Orientation::from_exif)
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    picture.apply_orientation(turn);
    Some((picture, (w, h) == (width, height)))
}

/// Reads the picture and turns it the way the camera says it was held.
fn read_upright(source: &Path) -> Result<image::DynamicImage, String> {
    use image::ImageDecoder;
    match kind_of(source) {
        Some(Kind::Video) => return read_video(source),
        _ if needs_reader(source) => return read_iphone(source),
        _ => {}
    }
    let failed = || "Keel could not make a small copy of that picture.".to_string();
    let reader = image::ImageReader::open(source)
        .map_err(|_| failed())?
        .with_guessed_format()
        .map_err(|_| failed())?;
    let mut decoder = reader.into_decoder().map_err(|_| failed())?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut picture = image::DynamicImage::from_decoder(decoder).map_err(|_| failed())?;
    picture.apply_orientation(orientation);
    Ok(picture)
}

/// A program on this computer that Keel uses to read a kind of file it cannot
/// read itself, and the packages that bring it.
struct Helper {
    names: &'static [&'static str],
    packages: &'static str,
    waiting: &'static str,
    failed: &'static str,
    asked: &'static Mutex<Option<Instant>>,
}

static IPHONE_ASKED: Mutex<Option<Instant>> = Mutex::new(None);
static VIDEO_ASKED: Mutex<Option<Instant>> = Mutex::new(None);

/// iPhone pictures (HEIC). The reader turns the picture upright as it reads it.
const IPHONE_READER: Helper = Helper {
    names: &["heif-dec", "heif-convert"],
    packages: "libheif-examples libheif-plugin-libde265|libheif-examples",
    waiting: "Keel is adding what it needs to show iPhone pictures. Try again in a few minutes.",
    failed: "Keel could not read that iPhone picture.",
    asked: &IPHONE_ASKED,
};

/// Videos: one frame becomes the video's picture. ffmpeg turns it upright.
const VIDEO_READER: Helper = Helper {
    names: &["ffmpeg"],
    packages: "ffmpeg",
    waiting: "Keel is adding what it needs to show videos. Try again in a few minutes.",
    failed: "Keel could not take a picture from that video.",
    asked: &VIDEO_ASKED,
};

/// A helper that hangs on a broken file is stopped after this long.
const HELPER_LIMIT: Duration = Duration::from_secs(90);

fn read_iphone(source: &Path) -> Result<image::DynamicImage, String> {
    let reader = IPHONE_READER.find()?;
    IPHONE_READER.picture(|out| {
        let mut command = std::process::Command::new(&reader);
        command.arg("-q").arg("92").arg(source).arg(out);
        command
    })
}

fn read_video(source: &Path) -> Result<image::DynamicImage, String> {
    let ffmpeg = VIDEO_READER.find()?;
    let frame_at = |second: &str| {
        let ffmpeg = ffmpeg.clone();
        let second = second.to_string();
        move |out: &Path| {
            let mut command = std::process::Command::new(&ffmpeg);
            command
                .args(["-nostdin", "-loglevel", "error", "-y", "-ss", &second, "-i"])
                .arg(source)
                .args(["-frames:v", "1", "-vf", "scale='min(1920,iw)':-2", "-q:v", "3"])
                .arg(out);
            command
        }
    };
    // A second in, past a fade from black; a very short video from its start.
    VIDEO_READER
        .picture(frame_at("1"))
        .or_else(|_| VIDEO_READER.picture(frame_at("0")))
}

impl Helper {
    fn find(&self) -> Result<PathBuf, String> {
        let dirs = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .unwrap_or_default();
        for name in self.names {
            for dir in dirs.iter().map(PathBuf::as_path).chain([Path::new("/usr/bin")]) {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
        self.ask();
        Err(self.waiting.into())
    }

    /// Runs the helper to write a JPEG, then reads it.
    fn picture(&self, command: impl FnOnce(&Path) -> std::process::Command) -> Result<image::DynamicImage, String> {
        let out = std::env::temp_dir().join(format!(
            "keel-read-{}-{}.jpg",
            std::process::id(),
            CONVERTED.fetch_add(1, Ordering::Relaxed)
        ));
        let mut command = command(&out);
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let finished = command.spawn().ok().and_then(|mut child| {
            let started = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => return Some(status.success()),
                    Ok(None) if started.elapsed() < HELPER_LIMIT => std::thread::sleep(Duration::from_millis(50)),
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                }
            }
        });
        let picture = if finished == Some(true) { image::open(&out).ok() } else { None };
        let _ = fs::remove_file(&out);
        picture.ok_or_else(|| self.failed.to_string())
    }

    /// Installs the helper in the background, at most once an hour.
    fn ask(&self) {
        if !cfg!(target_os = "linux") || cfg!(test) {
            return;
        }
        {
            let mut asked = self.asked.lock().unwrap_or_else(|poison| poison.into_inner());
            if asked.is_some_and(|when| when.elapsed() < HELPER_RETRY) {
                return;
            }
            *asked = Some(Instant::now());
        }
        // Each choice is a set of packages to try in turn.
        let tries: Vec<String> = self.packages.split('|').map(str::to_string).collect();
        std::thread::spawn(move || {
            let mut script = String::from(
                "set -uo pipefail\nexport DEBIAN_FRONTEND=noninteractive\nget() { apt-get install -y --no-install-recommends -o Acquire::ForceIPv4=true \"$@\" >/dev/null 2>&1; }\n",
            );
            for packages in &tries {
                script.push_str(&format!("get {packages} && exit 0\n"));
            }
            script.push_str("apt-get update -o Acquire::ForceIPv4=true >/dev/null 2>&1 || true\n");
            for packages in &tries {
                script.push_str(&format!("get {packages} && exit 0\n"));
            }
            script.push_str("echo \"Keel could not add a program it needs. Check the network.\" >&2\nexit 1\n");
            if let Err(message) = crate::storage::host("exec /bin/bash -s", &[], Some(&script)) {
                eprintln!("keel: {message}");
            }
        });
    }
}

fn save_jpeg(picture: &image::DynamicImage, dest: &Path, quality: u8) -> Result<(), String> {
    let failed = || "Keel could not keep a small copy on the disk.".to_string();
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| failed())?;
    }
    let tmp = dest.with_extension("part");
    let file = fs::File::create(&tmp).map_err(|_| failed())?;
    let mut writer = std::io::BufWriter::new(file);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
    picture.write_with_encoder(encoder).map_err(|_| failed())?;
    drop(writer);
    fs::rename(&tmp, dest).map_err(|_| failed())
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
        if !meta.is_file() {
            continue;
        }
        let Some(kind) = kind_of(&path) else {
            continue;
        };
        let Some(rel) = path.strip_prefix(root).ok().and_then(|rel| rel.to_str()) else {
            continue;
        };
        let rel = rel.replace('\\', "/");
        if rel.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
            continue;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("picture");
        out.push(entry(name, rel, kind, &meta));
    }
}

fn extension(path: &Path) -> Option<String> {
    path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase())
}

fn kind_of(path: &Path) -> Option<Kind> {
    match extension(path)?.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" => Some(Kind::Picture),
        "mp4" | "m4v" | "mov" | "webm" => Some(Kind::Video),
        _ => None,
    }
}

/// Pictures the image library cannot read itself.
fn needs_reader(path: &Path) -> bool {
    matches!(extension(path).as_deref(), Some("heic" | "heif"))
}

#[cfg(test)]
mod tests {
    use super::{
        categories_in, file_in, in_library, library, limits_for, list_in, open_original, open_sized_in, pictures_for, plan_copy_in,
        take_next, tidy_in, Job, Size,
    };
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
        assert!(open_original("../secret.jpg").is_err());
        assert!(open_original("Audiobooks/cover.jpg").is_err());
        let _ = fs::remove_dir_all(&media);
    }

    #[test]
    fn a_small_copy_is_kept_on_the_files_disk() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-preview-{stamp}"));
        let media = disk.join("Media");
        fs::create_dir_all(media.join("Pictures")).unwrap();
        image::RgbaImage::from_pixel(32, 32, image::Rgba([9, 8, 7, 128]))
            .save(media.join("Pictures").join("a.png"))
            .unwrap();
        let opened = open_sized_in(&media, "Pictures/a.png", Size::Small).unwrap();
        assert_eq!(opened.kind, "image/jpeg");
        assert!(opened.len > 0);
        // A picture read whole keeps all three copies from that one read.
        let saved = fs::read_dir(disk.join(".keel").join("previews")).unwrap().count();
        assert_eq!(saved, 3);
        assert!(!media.join(".keel").exists());
        let again = open_sized_in(&media, "Pictures/a.png", Size::Screen).unwrap();
        assert_eq!(again.kind, "image/jpeg");
        let _ = fs::remove_dir_all(&disk);
    }

    #[test]
    fn an_open_picture_is_a_screen_copy_not_the_whole_camera_picture() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-screen-{stamp}"));
        let media = disk.join("Media");
        fs::create_dir_all(media.join("Pictures")).unwrap();
        image::RgbImage::from_pixel(4000, 1000, image::Rgb([200, 100, 50]))
            .save(media.join("Pictures").join("wide.jpg"))
            .unwrap();
        let legacy = disk.join(".keel").join("previews").join(format!("{}.jpg", super::preview_name("Pictures/wide.jpg")));
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, b"old").unwrap();
        open_sized_in(&media, "Pictures/wide.jpg", Size::Screen).unwrap();
        let screen = image::open(super::preview_file(&media, "Pictures/wide.jpg", Size::Screen)).unwrap();
        assert_eq!((screen.width(), screen.height()), (1920, 480));
        let small = image::open(super::preview_file(&media, "Pictures/wide.jpg", Size::Small)).unwrap();
        assert_eq!((small.width(), small.height()), (480, 120));
        assert!(!legacy.exists());
        let _ = fs::remove_dir_all(&disk);
    }

    #[test]
    fn cleaning_drops_left_over_copies_then_the_oldest() {
        use std::time::{Duration, SystemTime};
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-tidy-{stamp}"));
        let media = disk.join("Media");
        fs::create_dir_all(media.join("Pictures").join("Trip")).unwrap();
        for name in ["a", "b", "c"] {
            image::RgbImage::from_pixel(64, 64, image::Rgb([1, 2, 3]))
                .save(media.join("Pictures").join("Trip").join(format!("{name}.jpg")))
                .unwrap();
            open_sized_in(&media, &format!("Pictures/Trip/{name}.jpg"), Size::Small).unwrap();
        }
        let previews = disk.join(".keel").join("previews");
        assert_eq!(fs::read_dir(&previews).unwrap().count(), 9);
        // A picture moved away leaves its copies behind.
        fs::remove_file(media.join("Pictures").join("Trip").join("c.jpg")).unwrap();
        let half = previews.join("leftover.part");
        fs::write(&half, b"half").unwrap();
        let old = SystemTime::now() - Duration::from_secs(3 * 24 * 60 * 60);
        fs::File::options().write(true).open(&half).unwrap().set_modified(old).unwrap();
        let tidied = tidy_in(&media, u64::MAX, u64::MAX);
        assert_eq!(tidied.removed, 4);
        assert_eq!(fs::read_dir(&previews).unwrap().count(), 6);
        // Over the limit, the least recently opened go first.
        let a_small = super::preview_file(&media, "Pictures/Trip/a.jpg", Size::Small);
        let a_screen = super::preview_file(&media, "Pictures/Trip/a.jpg", Size::Screen);
        let a_grid = super::preview_file(&media, "Pictures/Trip/a.jpg", Size::Grid);
        for path in [&a_small, &a_screen, &a_grid] {
            fs::File::options().write(true).open(path).unwrap().set_modified(old).unwrap();
        }
        let b_small = super::preview_file(&media, "Pictures/Trip/b.jpg", Size::Small);
        let b_screen = super::preview_file(&media, "Pictures/Trip/b.jpg", Size::Screen);
        let b_grid = super::preview_file(&media, "Pictures/Trip/b.jpg", Size::Grid);
        let b_total = [&b_small, &b_screen, &b_grid].iter().map(|path| fs::metadata(path).unwrap().len()).sum();
        let tidied = tidy_in(&media, b_total, b_total);
        assert_eq!(tidied.removed, 3);
        assert!(!a_small.exists() && !a_screen.exists() && !a_grid.exists());
        assert!(b_small.exists() && b_screen.exists());
        // With the library missing, nothing counts as left over.
        fs::remove_dir_all(&media).unwrap();
        assert_eq!(tidy_in(&media, u64::MAX, u64::MAX).removed, 0);
        assert!(b_small.exists());
        let _ = fs::remove_dir_all(&disk);
    }

    #[test]
    fn newest_pictures_come_first_by_the_camera_date() {
        use std::time::Duration;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-order-{stamp}"));
        let media = disk.join("Media");
        let trip = media.join("Pictures").join("Trip");
        fs::create_dir_all(&trip).unwrap();
        let day = |n: u64| UNIX_EPOCH + Duration::from_secs(1_700_000_000 + n * 86_400);
        for (name, n) in [("a.jpg", 1), ("b.jpg", 3), ("c.mp4", 2)] {
            fs::write(trip.join(name), b"x").unwrap();
            fs::File::options().write(true).open(trip.join(name)).unwrap().set_modified(day(n)).unwrap();
        }
        // Until the camera dates are read, the file dates count.
        let names = |pictures: Vec<super::Picture>| pictures.into_iter().map(|p| p.name).collect::<Vec<_>>();
        let first = pictures_for(&media, "Pictures/Trip").unwrap();
        assert!(first.iter().any(|p| p.name == "c.mp4" && p.video));
        assert_eq!(names(first), vec!["b.jpg", "c.mp4", "a.jpg"]);
        // a.jpg says it was taken in 2030, so it comes first once read.
        crate::picture_dates::wait_idle();
        fs::write(trip.join("a.jpg"), crate::picture_dates::tests::jpeg_with_date(true, "2030:05:06 07:08:09")).unwrap();
        let wanted: Vec<crate::picture_dates::Wanted> = ["a.jpg", "b.jpg", "c.mp4"]
            .iter()
            .map(|name| {
                let meta = fs::metadata(trip.join(name)).unwrap();
                crate::picture_dates::Wanted {
                    rel: format!("Pictures/Trip/{name}"),
                    file: trip.join(name),
                    modified: super::seconds(&meta),
                    len: meta.len(),
                }
            })
            .collect();
        crate::picture_dates::fill_now(&media, wanted);
        let (sorted, complete) = super::pictures_sorted(&media, "Pictures/Trip").unwrap();
        assert!(complete);
        assert_eq!(names(sorted), vec!["a.jpg", "b.jpg", "c.mp4"]);
        let _ = fs::remove_dir_all(&disk);
    }

    #[test]
    fn videos_are_listed_and_play_as_they_are() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-video-{stamp}"));
        let media = disk.join("Media");
        fs::create_dir_all(media.join("Pictures").join("Clips")).unwrap();
        fs::write(media.join("Pictures").join("Clips").join("party.MOV"), b"video").unwrap();
        fs::write(media.join("Pictures").join("phone.heic"), b"heic").unwrap();
        let categories = categories_in(&media);
        let clips = categories.iter().find(|c| c.name == "Clips").unwrap();
        assert_eq!(clips.cover, "");
        let loose = categories.iter().find(|c| c.path == "loose").unwrap();
        assert_eq!(loose.cover, "Pictures/phone.heic");
        assert!(!loose.cover_v.is_empty());
        // A video's copies are a frame from it, when the video tool is there.
        assert!(plan_copy_in(&media, "Pictures/Clips/party.MOV", Size::Small).is_ok());
        if super::VIDEO_READER.find().is_err() {
            let message = open_sized_in(&media, "Pictures/Clips/party.MOV", Size::Small).err().unwrap();
            assert!(message.contains("videos"), "{message}");
        }
        // Without the iPhone picture reader, a HEIC picture says so instead of failing quietly.
        if super::IPHONE_READER.find().is_err() {
            let message = open_sized_in(&media, "Pictures/phone.heic", Size::Small).err().unwrap();
            assert!(message.contains("iPhone"), "{message}");
        }
        let _ = fs::remove_dir_all(&disk);
    }

    #[test]
    fn an_open_picture_goes_first_then_the_newest_and_abandoned_ones_are_skipped() {
        let job = |name: &str, open: bool| {
            let (reply, answer) = tokio::sync::oneshot::channel();
            (
                Job {
                    source: name.into(),
                    copies: super::Copies {
                        grid: name.into(),
                        small: name.into(),
                        screen: name.into(),
                    },
                    size: if open { Size::Screen } else { Size::Grid },
                    open,
                    reply,
                },
                answer,
            )
        };
        let (opened, _opened_answer) = job("opened", true);
        let (old, _old_answer) = job("old", false);
        let (gone, gone_answer) = job("gone", false);
        let (new, _new_answer) = job("new", false);
        drop(gone_answer);
        let mut jobs = vec![opened, old, gone, new];
        let next = |jobs: &mut Vec<Job>| take_next(jobs).unwrap().source;
        assert_eq!(next(&mut jobs), std::path::PathBuf::from("opened"));
        assert_eq!(next(&mut jobs), std::path::PathBuf::from("new"));
        assert_eq!(next(&mut jobs), std::path::PathBuf::from("old"));
        assert!(take_next(&mut jobs).is_none());
    }

    #[test]
    fn a_big_jpeg_is_read_only_as_large_as_the_copy_needs() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let disk = std::env::temp_dir().join(format!("keel-scaled-{stamp}"));
        let media = disk.join("Media");
        fs::create_dir_all(media.join("Pictures")).unwrap();
        image::RgbImage::from_pixel(4000, 3000, image::Rgb([30, 60, 90]))
            .save(media.join("Pictures").join("big.jpg"))
            .unwrap();
        let (picture, whole) = super::read_jpeg_at(&media.join("Pictures").join("big.jpg"), 480).unwrap();
        assert_eq!((picture.width(), picture.height(), whole), (500, 375, false));
        // The grid needs only the small read; the screen copy waits for an open picture.
        open_sized_in(&media, "Pictures/big.jpg", Size::Grid).unwrap();
        let copy = |size| super::preview_file(&media, "Pictures/big.jpg", size);
        assert!(copy(Size::Grid).exists() && copy(Size::Small).exists() && !copy(Size::Screen).exists());
        let grid = image::open(copy(Size::Grid)).unwrap();
        assert_eq!((grid.width(), grid.height()), (240, 180));
        open_sized_in(&media, "Pictures/big.jpg", Size::Screen).unwrap();
        let screen = image::open(copy(Size::Screen)).unwrap();
        assert_eq!((screen.width(), screen.height()), (1920, 1440));
        // A grid copy missing beside an existing small copy is made from it.
        fs::remove_file(copy(Size::Grid)).unwrap();
        open_sized_in(&media, "Pictures/big.jpg", Size::Grid).unwrap();
        assert!(copy(Size::Grid).exists());
        let _ = fs::remove_dir_all(&disk);
    }

    #[test]
    fn the_camera_turn_is_read_from_a_jpeg() {
        let mut tiff = b"MM\0\x2a\0\0\0\x08\0\x01".to_vec();
        tiff.extend([0x01, 0x12, 0x00, 0x03, 0, 0, 0, 1, 0x00, 0x06, 0, 0]);
        assert_eq!(crate::picture_dates::orientation_in_tiff(&tiff), Some(6));
        assert_eq!(crate::picture_dates::orientation_in_tiff(b"nope"), None);
    }

    #[test]
    fn the_copies_take_a_share_of_the_disk() {
        const GB: u64 = 1024 * 1024 * 1024;
        assert_eq!(limits_for(1000 * GB, 500 * GB).0, 4 * GB);
        assert_eq!(limits_for(64 * GB, 30 * GB).0, 64 * GB / 50);
        assert_eq!(limits_for(8 * GB, 4 * GB).0, 512 * 1024 * 1024);
        let (limit, target) = limits_for(1000 * GB, 10 * GB);
        assert_eq!((limit, target), (GB, GB / 2));
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
        assert_eq!(categories[0].cover, "Pictures/Holiday/a.jpg");
        assert_eq!(categories[1].cover, "Pictures/loose.jpg");
        assert!(in_library(&categories[0].cover));
        let holiday = pictures_for(&media, "Pictures/Holiday").unwrap();
        assert_eq!(holiday[0].path, "Pictures/Holiday/a.jpg");
        let loose = pictures_for(&media, "loose").unwrap();
        assert_eq!(loose[0].path, "Pictures/loose.jpg");
        assert!(pictures_for(&media, "../secret").is_err());
        assert!(pictures_for(&media, "Audiobooks/cover").is_err());
        let _ = fs::remove_dir_all(&media);
    }
}
