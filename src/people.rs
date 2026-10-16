//! People who sign in to the page.
//!
//! A name is stored with a stretched hash of the password. The password itself
//! is never written. A sign-in records the name, so the page can say who it was.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const STRETCH: u32 = 50_000;
const VISIT_KEEP: usize = 30;

#[derive(Serialize, Deserialize, Clone)]
struct Person {
    name: String,
    salt: String,
    hash: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Visit {
    name: String,
    at_ms: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct Book {
    #[serde(default)]
    people: Vec<Person>,
    #[serde(default)]
    visits: Vec<Visit>,
}

#[derive(Serialize)]
pub struct PersonName {
    pub name: String,
}

#[derive(Serialize)]
pub struct VisitView {
    pub name: String,
    pub at_ms: u64,
}

#[derive(Serialize)]
pub struct PeopleView {
    pub people: Vec<PersonName>,
    pub visits: Vec<VisitView>,
}

fn file_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn people_path() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into())).join("people.json")
}

fn load(path: &std::path::Path) -> Book {
    let Ok(bytes) = fs::read(path) else {
        return Book::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn save(path: &std::path::Path, book: &Book) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "Keel could not save that person. Try again.".to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(book).map_err(|_| "Keel could not save that person. Try again.".to_string())?;
    fs::write(&tmp, &bytes).map_err(|_| "Keel could not save that person. Try again.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path).map_err(|_| "Keel could not save that person. Try again.".to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis() as u64)
        .unwrap_or(0)
}

fn stretch(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(password.as_bytes());
    let mut out = hasher.finalize();
    for _ in 0..STRETCH {
        let mut next = Sha256::new();
        next.update(&out);
        next.update(salt);
        out = next.finalize();
    }
    out.into()
}

fn same_bytes(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = (bytes[index] as char).to_digit(16)?;
        let low = (bytes[index + 1] as char).to_digit(16)?;
        out.push(((high << 4) | low) as u8);
        index += 2;
    }
    Some(out)
}

pub fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    let count = name.chars().count();
    if !(1..=40).contains(&count) {
        return Err("Use a short name.".into());
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '\'')
    {
        return Err("Use a name with letters, numbers, spaces, or a hyphen.".into());
    }
    Ok(name.to_string())
}

fn clean_password(password: &str) -> Result<(), String> {
    if password.chars().count() > 200 || password.trim().chars().count() < 8 {
        return Err("Use a password of at least 8 characters.".into());
    }
    Ok(())
}

fn find<'a>(book: &'a Book, name: &str) -> Option<&'a Person> {
    book.people.iter().find(|person| person.name.eq_ignore_ascii_case(name))
}

fn check_book(book: &Book, name: &str, password: &str) -> Result<String, String> {
    if book.people.is_empty() {
        return Err("No one can sign in yet. On the Keel screen, open Settings and add the first person.".into());
    }
    let Some(person) = find(book, name.trim()) else {
        return Err("That name or password is not right.".into());
    };
    let Some(salt) = hex_decode(&person.salt) else {
        return Err("That name or password is not right.".into());
    };
    let Some(expected) = hex_decode(&person.hash) else {
        return Err("That name or password is not right.".into());
    };
    let got = stretch(password, &salt);
    if !same_bytes(&got, &expected) {
        return Err("That name or password is not right.".into());
    }
    Ok(person.name.clone())
}

fn add_to(book: &mut Book, name: &str, password: &str) -> Result<(String, bool), String> {
    let name = clean_name(name)?;
    clean_password(password)?;
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|_| "Keel could not save that person. Try again.".to_string())?;
    let hash = stretch(password, &salt);
    let person = Person {
        name: name.clone(),
        salt: hex_encode(&salt),
        hash: hex_encode(&hash),
    };
    if let Some(existing) = book
        .people
        .iter_mut()
        .find(|item| item.name.eq_ignore_ascii_case(&name))
    {
        let kept = existing.name.clone();
        existing.salt = person.salt;
        existing.hash = person.hash;
        return Ok((kept, true));
    }
    book.people.push(person);
    book.people.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok((name, false))
}

fn note(book: &mut Book, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    book.visits.insert(0, Visit { name: name.to_string(), at_ms: now_ms() });
    book.visits.truncate(VISIT_KEEP);
}

pub fn sign_in(name: &str, password: &str) -> Result<String, String> {
    let _lock = file_lock();
    check_book(&load(&people_path()), name, password)
}

pub fn add_person(name: &str, password: &str) -> Result<String, String> {
    let _lock = file_lock();
    let path = people_path();
    let mut book = load(&path);
    let (kept, replaced) = add_to(&mut book, name, password)?;
    save(&path, &book)?;
    if replaced {
        Ok(format!("{kept}'s password is replaced."))
    } else {
        Ok(format!("{kept} can sign in."))
    }
}

pub fn remove_person(name: &str) -> Result<String, String> {
    let name = clean_name(name)?;
    let _lock = file_lock();
    let path = people_path();
    let mut book = load(&path);
    let before = book.people.len();
    book.people.retain(|person| !person.name.eq_ignore_ascii_case(&name));
    if book.people.len() == before {
        return Err("That name is not on this computer.".into());
    }
    save(&path, &book)?;
    Ok(format!("{name} can no longer sign in."))
}

pub fn view() -> PeopleView {
    let _lock = file_lock();
    let book = load(&people_path());
    PeopleView {
        people: book
            .people
            .iter()
            .map(|person| PersonName { name: person.name.clone() })
            .collect(),
        visits: book
            .visits
            .iter()
            .map(|visit| VisitView {
                name: visit.name.clone(),
                at_ms: visit.at_ms,
            })
            .collect(),
    }
}

pub fn remember(name: &str) {
    let _lock = file_lock();
    let path = people_path();
    let mut book = load(&path);
    note(&mut book, name);
    let _ = save(&path, &book);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_and_password_match_only_that_person() {
        let mut book = Book::default();
        let (name, replaced) = add_to(&mut book, "Robby", "correct horse").unwrap();
        assert_eq!(name, "Robby");
        assert!(!replaced);
        assert_eq!(check_book(&book, "robby", "correct horse").unwrap(), "Robby");
        assert!(check_book(&book, "Robby", "wrong password").is_err());
        assert!(check_book(&book, "Someone", "correct horse").is_err());
        let (kept, replaced) = add_to(&mut book, "robby", "another password here").unwrap();
        assert_eq!(kept, "Robby");
        assert!(replaced);
        assert_eq!(book.people.len(), 1);
        assert!(check_book(&book, "Robby", "correct horse").is_err());
        assert_eq!(check_book(&book, "Robby", "another password here").unwrap(), "Robby");
        add_to(&mut book, "Ada", "a different secret").unwrap();
        assert_eq!(book.people.len(), 2);
        assert!(check_book(&Book::default(), "Robby", "another password here")
            .unwrap_err()
            .contains("Keel screen"));
    }

    #[test]
    fn names_and_passwords_have_a_plain_shape() {
        assert!(clean_name("Robby").is_ok());
        assert!(clean_name("Ada Lovelace").is_ok());
        assert!(clean_name("").is_err());
        assert!(clean_name("a/b").is_err());
        assert!(clean_password("short").is_err());
        assert!(clean_password("long enough").is_ok());
        let mut book = Book::default();
        note(&mut book, "Robby");
        note(&mut book, "Ada");
        assert_eq!(book.visits[0].name, "Ada");
        assert_eq!(book.visits.len(), 2);
    }

    #[test]
    fn the_saved_file_does_not_contain_the_password() {
        let dir = std::env::temp_dir().join(format!(
            "keel-people-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("people.json");
        let mut book = Book::default();
        add_to(&mut book, "Robby", "correct horse").unwrap();
        save(&path, &book).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("correct horse"));
        assert!(text.contains("Robby"));
        let loaded = load(&path);
        assert_eq!(check_book(&loaded, "Robby", "correct horse").unwrap(), "Robby");
        let _ = fs::remove_dir_all(&dir);
    }
}
