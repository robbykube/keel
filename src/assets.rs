//! The browser keeps scripts and styles until they change.
//!
//! Pages name each script and style with a short fingerprint of the file, and
//! a file asked for by fingerprint may be kept for good: a changed file gets a
//! new fingerprint, so a page never draws with an old script after an update.
//! Pages themselves are still checked every time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};

/// Pages are small; anything larger is passed through untouched.
const PAGE_LIMIT: usize = 2 * 1024 * 1024;

static ROOT: OnceLock<PathBuf> = OnceLock::new();
/// File → (modified, size, fingerprint), so a file is read again only when it changes.
static PRINTS: Mutex<Option<HashMap<PathBuf, (SystemTime, u64, String)>>> = Mutex::new(None);

pub fn serve_from(root: PathBuf) {
    let _ = ROOT.set(root);
}

pub async fn versioned(mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if path.starts_with("/api/") {
        return next.run(request).await;
    }
    let asset = path.ends_with(".js") || path.ends_with(".css");
    let page = !asset && (path.ends_with(".html") || path.ends_with('/') || !path.rsplit('/').next().unwrap_or("").contains('.'));
    if page {
        // A page's fingerprints can change while the page does not, so a
        // remembered copy of the page is never good enough.
        request.headers_mut().remove(IF_MODIFIED_SINCE);
        request.headers_mut().remove(IF_NONE_MATCH);
    }
    let fingerprinted = asset && request.uri().query().is_some_and(|query| query.starts_with("v="));
    let mut response = next.run(request).await;
    if fingerprinted && response.status().is_success() {
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
        return response;
    }
    let html = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if !page || !html || !response.status().is_success() {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, PAGE_LIMIT).await else {
        return Response::from_parts(parts, Body::empty());
    };
    let Some(root) = ROOT.get() else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let text = String::from_utf8_lossy(&bytes);
    let rewritten = add_prints(&text, |asset| print_of(root, asset));
    parts.headers.remove(ETAG);
    parts.headers.remove(LAST_MODIFIED);
    if let Ok(len) = HeaderValue::from_str(&rewritten.len().to_string()) {
        parts.headers.insert(CONTENT_LENGTH, len);
    }
    Response::from_parts(parts, Body::from(rewritten))
}

/// Adds `?v=<fingerprint>` to each local script and style the page names.
pub fn add_prints(page: &str, print: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(page.len() + 256);
    let mut rest = page;
    loop {
        let next = ["src=\"/", "href=\"/"]
            .iter()
            .filter_map(|marker| rest.find(marker).map(|at| (at, marker.len())))
            .min_by_key(|(at, _)| *at);
        let Some((at, len)) = next else {
            out.push_str(rest);
            return out;
        };
        let start = at + len - 1;
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find('"').unwrap_or(rest.len());
        let link = &rest[..end];
        out.push_str(link);
        let local = !link.starts_with("//") && !link.contains('?') && !link.contains('#');
        if local && (link.ends_with(".js") || link.ends_with(".css")) {
            if let Some(print) = print(link) {
                out.push_str("?v=");
                out.push_str(&print);
            }
        }
        rest = &rest[end..];
    }
}

fn print_of(root: &Path, link: &str) -> Option<String> {
    let rel = link.trim_start_matches('/');
    if rel.is_empty() || rel.split('/').any(|part| part.is_empty() || part == "..") {
        return None;
    }
    let file = root.join(rel);
    let meta = std::fs::metadata(&file).ok()?;
    let modified = meta.modified().ok()?;
    let mut guard = PRINTS.lock().unwrap_or_else(|poison| poison.into_inner());
    let prints = guard.get_or_insert_with(HashMap::new);
    if let Some((seen, len, print)) = prints.get(&file) {
        if *seen == modified && *len == meta.len() {
            return Some(print.clone());
        }
    }
    let bytes = std::fs::read(&file).ok()?;
    let digest = Sha256::digest(&bytes);
    let print: String = digest[..5].iter().map(|byte| format!("{byte:02x}")).collect();
    prints.insert(file, (modified, meta.len(), print.clone()));
    Some(print)
}

#[cfg(test)]
mod tests {
    #[test]
    fn local_scripts_and_styles_get_a_fingerprint() {
        let page = r#"<link rel="stylesheet" href="/style.css" /><script src="/chrome.js"></script>
<a href="/shares.html">Files</a><script src="https://example.com/x.js"></script><img src="/logo.png"><script src="/done.js?v=1"></script>"#;
        let out = super::add_prints(page, |link| Some(format!("p{}", link.len())));
        assert!(out.contains(r#"href="/style.css?v=p10""#), "{out}");
        assert!(out.contains(r#"src="/chrome.js?v=p10""#), "{out}");
        assert!(out.contains(r#"href="/shares.html""#));
        assert!(out.contains(r#"src="https://example.com/x.js""#));
        assert!(out.contains(r#"src="/logo.png""#));
        assert!(out.contains(r#"src="/done.js?v=1""#));
        assert_eq!(super::add_prints("no links", |_| None), "no links");
    }
}
