use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use flate2::read::GzDecoder;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const REPOSITORY: &str = "robbykube/keel";
const MAX_CHECKSUM_BYTES: usize = 64 * 1024;
const MAX_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const RELEASE_PUBLIC_KEY_HEX: &str = include_str!("../packaging/updates/release-public.hex");
const BAKED_GITHUB_CLIENT_ID: &str = include_str!("../packaging/updates/github-client-id");

#[derive(Serialize)]
pub struct UpdateStatus {
    pub state: &'static str,
    pub current: &'static str,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub published_at: Option<String>,
    pub appliance_asset: Option<String>,
    pub checksum_asset: Option<String>,
    pub signature: &'static str,
    pub can_apply: bool,
    pub can_restore: bool,
    pub github_token_set: bool,
    pub github_can_connect: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct UpdateRequest {
    pub confirmation: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct UpdateProgress {
    pub state: String,
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

impl UpdateProgress {
    fn idle() -> Self {
        Self {
            state: "idle".into(),
            stage: "idle".into(),
            percent: 0,
            message: String::new(),
        }
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    url: String,
    browser_download_url: String,
}

struct HostCaps {
    linux: bool,
    live: bool,
    helper: bool,
}

struct SignatureCheck {
    state: &'static str,
    sha256: Option<String>,
}

pub async fn check() -> UpdateStatus {
    let current = env!("CARGO_PKG_VERSION");
    let caps = host_caps();
    let restore = can_restore();
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let client = match github_client() {
        Ok(client) => client,
        Err(error) => {
            return unavailable(
                current,
                restore,
                format!("could not prepare the update check: {error}"),
            )
        }
    };

    let response = match client
        .get(&url)
        .header(ACCEPT, HeaderValue::from_static("application/vnd.github+json"))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return unavailable(
                current,
                restore,
                format!("GitHub could not be reached: {error}"),
            )
        }
    };
    if !allowed_download_url(response.url()) {
        return unavailable(
            current,
            restore,
            "GitHub sent an unexpected address.".into(),
        );
    }
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return unavailable(
            current,
            restore,
            "No published Keel release is visible. Connect GitHub in Settings so this computer can see the private repository.".into(),
        );
    }
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return unavailable(
            current,
            restore,
            "GitHub refused this computer's sign-in. Connect GitHub again in Settings.".into(),
        );
    }
    if !response.status().is_success() {
        return unavailable(
            current,
            restore,
            format!("GitHub returned HTTP {}.", response.status().as_u16()),
        );
    }

    let release = match response.json::<GithubRelease>().await {
        Ok(release) => release,
        Err(error) => {
            return unavailable(
                current,
                restore,
                format!("GitHub returned an unreadable release: {error}"),
            )
        }
    };
    if release.draft || release.prerelease {
        return unavailable(
            current,
            restore,
            "The latest GitHub release is not a stable release.".into(),
        );
    }

    let latest = release.tag_name.trim_start_matches('v').to_string();
    let update_available = newer_than(&latest, current);
    let package = release
        .assets
        .iter()
        .find(|asset| is_package_name(&asset.name));
    let checksum = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS");
    let signature = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS.sig");

    let checked = if update_available {
        verify_release_assets(&client, package, checksum, signature).await
    } else {
        SignatureCheck {
            state: "unchecked",
            sha256: None,
        }
    };
    let decision = decide(current, &latest, update_available, checked.state, &caps);

    UpdateStatus {
        state: "checked",
        current,
        latest: Some(latest),
        update_available,
        release_url: Some(release.html_url),
        published_at: release.published_at,
        appliance_asset: package
            .filter(|_| checked.state == "verified")
            .map(|asset| asset.browser_download_url.clone()),
        checksum_asset: checksum.map(|asset| asset.browser_download_url.clone()),
        signature: checked.state,
        can_apply: decision.can_apply && checked.sha256.is_some(),
        can_restore: restore,
        github_token_set: github_token_is_set(),
        github_can_connect: github_can_connect(),
        message: decision.message,
    }
}

pub async fn queue_apply(confirmation: &str) -> Result<String, String> {
    let result = queue(confirmation, "apply", "INSTALL UPDATE").await;
    if result.is_ok() {
        write_update_progress(
            "queued",
            "queued",
            1,
            "The update is queued and will start shortly.",
        );
    }
    result
}

pub fn progress() -> UpdateProgress {
    if !cfg!(target_os = "linux") {
        return UpdateProgress::idle();
    }
    let path = data_dir().join("updates").join("progress.json");
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(UpdateProgress::idle)
}

pub async fn queue_restore(confirmation: &str) -> Result<String, String> {
    if confirmation != "RESTORE PREVIOUS" {
        return Err("Type RESTORE PREVIOUS to put the saved Keel back.".into());
    }
    if !cfg!(target_os = "linux") {
        return Err("Restore the previous version on the Keel machine.".into());
    }
    if Path::new("/run/live/medium").exists() {
        return Err("This is a USB trial. Nothing here is kept after shutdown.".into());
    }
    if !can_restore() {
        return Err("No previous Keel is saved on this machine.".into());
    }
    if !Path::new("/usr/lib/keel/rollback-update").is_file() {
        return Err("This machine does not have the restore program yet.".into());
    }
    write_request("rollback")?;
    write_update_progress(
        "queued",
        "queued",
        8,
        "Restoring the previous Keel.",
    );
    Ok("Restore requested. Keel will put the previous version back and restart.".into())
}

pub async fn apply_from_request() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("Updates install on the Linux Keel machine.".into());
    }
    let request_path = data_dir().join("updates").join("request");
    let request = fs::read_to_string(&request_path).unwrap_or_default();
    let _ = fs::remove_file(&request_path);
    match request.trim() {
        "rollback" => restore_previous(),
        "apply" => apply_verified_release().await,
        _ => Err("No update request was waiting.".into()),
    }
}

pub fn generate_release_key(private_path: &Path) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let absolute = if private_path.is_absolute() {
        private_path.to_path_buf()
    } else {
        cwd.join(private_path)
    };
    if absolute.starts_with(&cwd) {
        return Err("Refusing to write the private signing key inside the repository.".into());
    }
    let mut secret = [0u8; 32];
    getrandom::getrandom(&mut secret).map_err(|error| error.to_string())?;
    let signing = SigningKey::from_bytes(&secret);
    let public = encode_hex(signing.verifying_key().as_bytes());
    let public_path = Path::new("packaging/updates/release-public.hex");
    if let Some(parent) = public_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(public_path, format!("{public}\n")).map_err(|error| error.to_string())?;
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&absolute, format!("{}\n", encode_hex(&secret)))
        .map_err(|error| error.to_string())?;
    println!("Public key written to {}", public_path.display());
    println!("Private key written outside the repository. It is not shown here.");
    Ok(())
}

pub fn sign_file(path: &Path) -> Result<(), String> {
    let key = signing_key_from_env()?;
    let message =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let signature = key.sign(&message);
    release_public_key()?
        .verify(&message, &signature)
        .map_err(|_| {
            "The signing key does not match the public key built into this Keel.".to_string()
        })?;
    let signature_path = format!("{}.sig", path.display());
    fs::write(
        &signature_path,
        format!("{}\n", encode_hex(&signature.to_bytes())),
    )
    .map_err(|error| format!("could not write {signature_path}: {error}"))?;
    println!("Signed {signature_path}");
    Ok(())
}

async fn queue(confirmation: &str, request: &str, expected: &str) -> Result<String, String> {
    if confirmation != expected {
        return Err(format!("Type {expected} to continue."));
    }
    if request != "apply" {
        return Err("Unknown update request.".into());
    }
    if !cfg!(target_os = "linux") {
        return Err("Install the update on the Keel machine. This copy only checks GitHub.".into());
    }
    let status = check().await;
    if !status.can_apply {
        return Err(status.message);
    }
    write_request("apply")?;
    Ok("Update requested. Keel will install it and restart. If the new version does not start, the previous version is restored.".into())
}

fn write_request(kind: &str) -> Result<(), String> {
    let dir = data_dir().join("updates");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("could not prepare the update folder: {error}"))?;
    let path = dir.join("request");
    let temp = dir.join("request.new");
    fs::write(&temp, format!("{kind}\n"))
        .map_err(|error| format!("could not write the update request: {error}"))?;
    fs::rename(&temp, &path)
        .map_err(|error| format!("could not save the update request: {error}"))?;
    Ok(())
}

async fn apply_verified_release() -> Result<String, String> {
    write_update_progress("running", "starting", 2, "Starting the signed update.");
    let result = apply_verified_release_inner().await;
    match &result {
        Ok(message) => write_update_progress("done", "complete", 100, message),
        Err(error) => write_update_progress("error", "stopped", 0, error),
    }
    result
}

async fn apply_verified_release_inner() -> Result<String, String> {
    if Path::new("/run/live/medium").exists() {
        return Err(
            "This is a USB trial. Install Keel on the internal disk before updating.".into(),
        );
    }
    clear_result();
    write_update_progress(
        "running",
        "checking",
        5,
        "Checking GitHub for the latest signed release.",
    );
    let client = github_client()?;
    let release = fetch_latest(&client).await?;
    let package = release
        .assets
        .iter()
        .find(|asset| is_package_name(&asset.name))
        .ok_or_else(|| {
            "This release has no Keel package for this computer. A Pi needs the arm64 file.".to_string()
        })?;
    let checksum = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS")
        .ok_or_else(|| "The release has no checksum list.".to_string())?;
    let signature = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS.sig")
        .ok_or_else(|| "The release is not signed, so Keel will not install it.".to_string())?;
    let sums = read_limited_with_progress(
        &client,
        asset_download_url(checksum),
        MAX_CHECKSUM_BYTES,
        "Downloading the signed checksum.",
        8,
        16,
    )
    .await?;
    let sig = read_limited_with_progress(
        &client,
        asset_download_url(signature),
        512,
        "Downloading the signature.",
        16,
        22,
    )
    .await?;
    write_update_progress(
        "running",
        "verifying",
        26,
        "Checking the release signature.",
    );
    verify_bytes(&sums, &sig)?;
    let expected = checksum_for(&String::from_utf8_lossy(&sums), &package.name)
        .ok_or_else(|| "The signed checksum list does not include the Keel package.".to_string())?;
    let package_bytes = read_limited_with_progress(
        &client,
        asset_download_url(package),
        MAX_PACKAGE_BYTES,
        "Downloading the Keel update.",
        30,
        68,
    )
    .await?;
    write_update_progress(
        "running",
        "verifying",
        72,
        "Checking the downloaded update.",
    );
    let actual = encode_hex(&Sha256::digest(&package_bytes));
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err("The downloaded package does not match the signed checksum.".into());
    }

    let updates = data_dir().join("updates");
    let staged = updates.join("staged");
    let _ = fs::remove_dir_all(&staged);
    fs::create_dir_all(&staged).map_err(|error| error.to_string())?;
    write_update_progress("running", "staging", 76, "Preparing the update.");
    extract_package(&package_bytes, &staged)?;
    write_update_progress("running", "saving", 80, "Saving the current Keel version.");
    save_previous()?;
    write_pending()?;
    write_update_progress("running", "installing", 86, "Installing the signed update.");
    if let Err(error) = install_staged(&staged) {
        let _ = run_rollback_script();
        let _ = restart_keel();
        return Err(format!(
            "The update was not installed. The previous version is still in place. {error}"
        ));
    }
    // The boot restore is ordered before Keel and sees the unfinished-update
    // marker. A restart would run it and put the old program back before this
    // new one can answer. A runtime mask lasts until reboot, so a power cut
    // still restores.
    let _ = hold_boot_rollback();
    write_update_progress("running", "restarting", 92, "Restarting Keel.");
    if let Err(error) = restart_keel() {
        let _ = run_rollback_script();
        let _ = restart_keel();
        release_boot_rollback();
        return Err(format!(
            "Keel did not restart. The previous version was restored. {error}"
        ));
    }
    write_update_progress(
        "running",
        "checking",
        96,
        "Waiting for the new Keel to answer.",
    );
    let expected_version = release.tag_name.trim_start_matches('v');
    if let Err(error) = wait_for_version(expected_version).await {
        let _ = run_rollback_script();
        let _ = restart_keel();
        release_boot_rollback();
        let message =
            format!("The new Keel did not start, so the previous version was restored. {error}");
        write_result(&message);
        return Err(message);
    }
    clear_pending();
    release_boot_rollback();
    let message = format!("Keel {expected_version} is running. The previous version is still available if you need it.");
    write_result(&message);
    Ok(message)
}

fn restore_previous() -> Result<String, String> {
    write_update_progress("running", "restoring", 40, "Putting the previous Keel back.");
    if let Err(error) = run_rollback_script() {
        write_update_progress("error", "stopped", 0, &error);
        return Err(error);
    }
    write_update_progress("running", "restarting", 80, "Restarting Keel.");
    if let Err(error) = restart_keel() {
        write_update_progress("error", "stopped", 0, &error);
        return Err(error);
    }
    let message = "The previous Keel is running again.";
    write_result(message);
    write_update_progress("done", "complete", 100, message);
    Ok(message.into())
}

async fn verify_release_assets(
    client: &reqwest::Client,
    package: Option<&GithubAsset>,
    checksum: Option<&GithubAsset>,
    signature: Option<&GithubAsset>,
) -> SignatureCheck {
    let (Some(package), Some(checksum), Some(signature)) = (package, checksum, signature) else {
        if package.is_none() && checksum.is_some() && signature.is_some() {
            return SignatureCheck {
                state: "no_package",
                sha256: None,
            };
        }
        return SignatureCheck {
            state: "missing",
            sha256: None,
        };
    };
    let sums = match read_limited(client, asset_download_url(checksum), MAX_CHECKSUM_BYTES).await {
        Ok(sums) => sums,
        Err(_) => {
            return SignatureCheck {
                state: "download_failed",
                sha256: None,
            }
        }
    };
    let sig = match read_limited(client, asset_download_url(signature), 512).await {
        Ok(sig) => sig,
        Err(_) => {
            return SignatureCheck {
                state: "download_failed",
                sha256: None,
            }
        }
    };
    if verify_bytes(&sums, &sig).is_err() {
        return SignatureCheck {
            state: "invalid",
            sha256: None,
        };
    }
    match checksum_for(&String::from_utf8_lossy(&sums), &package.name) {
        Some(sha256) => SignatureCheck {
            state: "verified",
            sha256: Some(sha256),
        },
        None => SignatureCheck {
            state: "invalid",
            sha256: None,
        },
    }
}

async fn fetch_latest(client: &reqwest::Client) -> Result<GithubRelease, String> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let response = client
        .get(url)
        .header(ACCEPT, HeaderValue::from_static("application/vnd.github+json"))
        .send()
        .await
        .map_err(|error| format!("GitHub could not be reached: {error}"))?;
    if !allowed_download_url(response.url()) {
        return Err("GitHub sent an unexpected address.".into());
    }
    if !response.status().is_success() {
        return Err(format!(
            "GitHub returned HTTP {}.",
            response.status().as_u16()
        ));
    }
    let release = response
        .json::<GithubRelease>()
        .await
        .map_err(|error| format!("GitHub returned an unreadable release: {error}"))?;
    if release.draft || release.prerelease {
        return Err("The latest GitHub release is not a stable release.".into());
    }
    let latest = release.tag_name.trim_start_matches('v');
    if !newer_than(latest, env!("CARGO_PKG_VERSION")) {
        return Err("This Keel is already the latest signed release.".into());
    }
    Ok(release)
}

async fn read_limited(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
) -> Result<Vec<u8>, String> {
    read_limited_with_progress(client, url, limit, "", 0, 0).await
}

async fn read_limited_with_progress(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
    message: &str,
    start: u8,
    end: u8,
) -> Result<Vec<u8>, String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| "The download address is not a URL.".to_string())?;
    if !allowed_download_url(&parsed) {
        return Err("Refusing a download that is not from GitHub.".into());
    }
    let mut response = client
        .get(parsed)
        .header(ACCEPT, HeaderValue::from_static("application/octet-stream"))
        .send()
        .await
        .map_err(download_error)?;
    if !allowed_download_url(response.url()) {
        return Err("The download was redirected away from GitHub.".into());
    }
    if !response.status().is_success() {
        return Err(format!(
            "download returned HTTP {}.",
            response.status().as_u16()
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(download_error)? {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err("The download is larger than Keel accepts.".into());
        }
        body.extend_from_slice(&chunk);
        if !message.is_empty() {
            let percent = response
                .content_length()
                .filter(|total| *total > 0)
                .map(|total| {
                    let fraction = (body.len() as u64).min(total) as u128 * 100
                        / total as u128;
                    start.saturating_add(
                        ((end.saturating_sub(start) as u128 * fraction) / 100) as u8,
                    )
                })
                .unwrap_or(start);
            write_update_progress("running", "downloading", percent, message);
        }
    }
    if !message.is_empty() {
        write_update_progress("running", "downloading", end, message);
    }
    Ok(body)
}

fn verify_bytes(message: &[u8], signature_text: &[u8]) -> Result<(), String> {
    let signature = decode_hex(&String::from_utf8_lossy(signature_text))?;
    let signature = Signature::from_slice(&signature)
        .map_err(|_| "The signature is the wrong size.".to_string())?;
    release_public_key()?
        .verify(message, &signature)
        .map_err(|_| "The signature does not match this Keel's trusted key.".to_string())
}

fn release_public_key() -> Result<VerifyingKey, String> {
    let bytes = decode_hex(RELEASE_PUBLIC_KEY_HEX.trim())?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "The built-in release key is the wrong size.".to_string())?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|_| "The built-in release key is not valid.".to_string())
}

fn signing_key_from_env() -> Result<SigningKey, String> {
    let text = std::env::var("KEEL_RELEASE_SIGNING_KEY")
        .map_err(|_| "KEEL_RELEASE_SIGNING_KEY is not set.".to_string())?;
    let bytes = decode_hex(text.trim())?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "The signing key is the wrong size.".to_string())?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn decide(
    current: &str,
    latest: &str,
    update_available: bool,
    signature: &str,
    caps: &HostCaps,
) -> Decision {
    if !update_available {
        return Decision {
            can_apply: false,
            message: format!("This Keel is up to date with GitHub release {latest}."),
        };
    }
    let message = match signature {
        "verified" if caps.can_apply() => format!(
            "Keel {latest} is available. The signature checks out. Install update keeps version {current} and restores it if the new one does not start."
        ),
        "verified" if !caps.linux => format!(
            "Keel {latest} is available and the signature checks out. Install it on the Keel machine."
        ),
        "verified" if caps.live => format!(
            "Keel {latest} is available and the signature checks out. Install Keel on the internal disk before updating."
        ),
        "verified" => format!(
            "Keel {latest} is available and the signature checks out. This machine does not have the update service yet, so Keel will not install it."
        ),
        "missing" => format!("Keel {latest} is available, but it is not signed. Keel will not install it."),
        "no_package" => format!(
            "Keel {latest} is on GitHub, but that release has no file for this kind of computer."
        ),
        "download_failed" => format!(
            "Keel {latest} is available, but this computer could not download the signed files from GitHub."
        ),
        _ => format!("Keel {latest} is available, but the signature is not valid. Keel will not install it."),
    };
    Decision {
        can_apply: signature == "verified" && caps.can_apply(),
        message,
    }
}

struct Decision {
    can_apply: bool,
    message: String,
}

impl HostCaps {
    fn can_apply(&self) -> bool {
        self.linux && self.helper && !self.live
    }
}

fn host_caps() -> HostCaps {
    HostCaps {
        linux: cfg!(target_os = "linux"),
        live: Path::new("/run/live/medium").exists(),
        helper: Path::new("/usr/lib/keel/rollback-update").is_file(),
    }
}

fn can_restore() -> bool {
    data_dir()
        .join("updates")
        .join("previous")
        .join("keel")
        .is_file()
}

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("KEEL_DATA").unwrap_or_else(|_| "data".into()))
}

fn save_previous() -> Result<(), String> {
    let updates = data_dir().join("updates");
    let staging = updates.join("previous-new");
    let previous = updates.join("previous");
    let retired = updates.join("previous-old");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(staging.join("web")).map_err(|error| error.to_string())?;
    fs::copy("/usr/bin/keel", staging.join("keel"))
        .map_err(|error| format!("could not save the current Keel: {error}"))?;
    copy_tree(Path::new("/usr/lib/keel/web"), &staging.join("web"))?;
    copy_data_snapshot(&staging.join("data"))?;
    if retired.exists() {
        fs::remove_dir_all(&retired).map_err(|error| error.to_string())?;
    }
    if previous.exists() {
        fs::rename(&previous, &retired)
            .map_err(|error| format!("could not move the saved Keel aside: {error}"))?;
    }
    if let Err(error) = fs::rename(&staging, &previous) {
        let _ = fs::rename(&retired, &previous);
        return Err(format!("could not save the current Keel: {error}"));
    }
    let _ = fs::remove_dir_all(&retired);
    Ok(())
}

fn copy_data_snapshot(dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|error| error.to_string())?;
    let data = data_dir();
    if !data.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&data).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.file_name() == "updates" {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            return Err("Refusing to save a link from the Keel data folder.".into());
        }
        let target = dest.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn install_staged(staged: &Path) -> Result<(), String> {
    let binary = staged.join("keel");
    let web = staged.join("web");
    if !binary.is_file() || !web.is_dir() {
        return Err("The package does not contain the Keel program and its pages.".into());
    }
    replace_file(&binary, Path::new("/usr/bin/keel"))?;
    replace_dir(&web, Path::new("/usr/lib/keel/web"))?;
    Ok(())
}

fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    let parent = to
        .parent()
        .ok_or_else(|| format!("{} has no folder", to.display()))?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(
        ".{}-new",
        to.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("keel")
    ));
    fs::copy(from, &temp).map_err(|error| format!("could not stage {}: {error}", to.display()))?;
    // Unpack creates the program as a normal file. systemd will say the
    // restart worked, then the program never answers, and the old Keel is
    // restored. The restore script already sets this bit; the install must too.
    mark_executable(&temp)?;
    sync_file(&temp)?;
    match fs::rename(&temp, to) {
        Ok(()) => Ok(()),
        Err(error) => {
            let backup = parent.join(format!(
                ".{}-old",
                to.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("keel")
            ));
            let _ = fs::remove_file(&backup);
            fs::rename(to, &backup).map_err(|move_error| {
                format!(
                    "could not replace {}: {error}; also could not move it aside: {move_error}",
                    to.display()
                )
            })?;
            if let Err(replace_error) = fs::rename(&temp, to) {
                let _ = fs::rename(&backup, to);
                return Err(format!(
                    "could not replace {}: {replace_error}",
                    to.display()
                ));
            }
            let _ = fs::remove_file(&backup);
            Ok(())
        }
    }
}

fn replace_dir(from: &Path, to: &Path) -> Result<(), String> {
    let parent = to
        .parent()
        .ok_or_else(|| format!("{} has no folder", to.display()))?;
    let incoming = parent.join(".web-incoming");
    let backup = parent.join(".web-backup");
    if incoming.exists() {
        fs::remove_dir_all(&incoming).map_err(|error| error.to_string())?;
    }
    copy_tree(from, &incoming)?;
    if backup.exists() {
        fs::remove_dir_all(&backup).map_err(|error| error.to_string())?;
    }
    if to.exists() {
        fs::rename(to, &backup)
            .map_err(|error| format!("could not move the current pages aside: {error}"))?;
    }
    if let Err(error) = fs::rename(&incoming, to) {
        let _ = fs::rename(&backup, to);
        return Err(format!("could not install the new pages: {error}"));
    }
    let _ = fs::remove_dir_all(&backup);
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(from).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            return Err(format!(
                "Refusing to copy a link: {}",
                entry.path().display()
            ));
        }
        let dest = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &dest).map_err(|error| error.to_string())?;
        } else {
            return Err(format!(
                "Refusing to copy a special file: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn extract_package(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    archive.set_preserve_permissions(false);
    for entry in archive
        .entries()
        .map_err(|error| format!("could not read the package: {error}"))?
    {
        let mut entry =
            entry.map_err(|error| format!("could not read a package entry: {error}"))?;
        let entry_type = entry.header().entry_type();
        if matches!(
            entry_type,
            tar::EntryType::XHeader
                | tar::EntryType::XGlobalHeader
                | tar::EntryType::GNULongName
                | tar::EntryType::GNULongLink
        ) {
            continue;
        }
        let path = entry
            .path()
            .map_err(|error| format!("could not read a package path: {error}"))?
            .into_owned();
        let relative = safe_member(&path)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(&relative);
        if entry_type.is_dir() {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            continue;
        }
        if !entry_type.is_file() {
            return Err(format!(
                "The package contains a link or special file: {}",
                relative.display()
            ));
        }
        if entry.size() > MAX_PACKAGE_BYTES as u64 {
            return Err("A file in the package is larger than Keel accepts.".into());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = File::create(&target).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| format!("could not unpack {}: {error}", relative.display()))?;
        if relative == Path::new("keel") {
            mark_executable(&target)?;
        }
    }
    if !dest.join("keel").is_file() || !dest.join("web").is_dir() {
        return Err("The package does not contain the Keel program and its pages.".into());
    }
    Ok(())
}

fn safe_member(path: &Path) -> Result<PathBuf, String> {
    let mut parts = path.components();
    match parts.next() {
        Some(Component::Normal(name)) if name == "keel" => {}
        _ => return Err("The package contains a file outside the Keel folder.".into()),
    }
    let mut relative = PathBuf::new();
    for part in parts {
        match part {
            Component::Normal(name) => relative.push(name),
            _ => return Err("The package contains a file outside the Keel folder.".into()),
        }
    }
    if relative.as_os_str().is_empty()
        || relative == Path::new("keel")
        || relative == Path::new("keel.service")
        || relative.starts_with("web")
    {
        if relative == Path::new("keel.service") {
            return Ok(PathBuf::from("keel.service"));
        }
        if relative == Path::new("keel") {
            return Ok(PathBuf::from("keel"));
        }
        if let Ok(rest) = relative.strip_prefix("web") {
            return Ok(Path::new("web").join(rest));
        }
        return Ok(relative);
    }
    Err(format!(
        "The package contains an unexpected file: {}",
        relative.display()
    ))
}

fn checksum_for(sums: &str, name: &str) -> Option<String> {
    for line in sums.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?;
        if Path::new(file).file_name().and_then(|value| value.to_str()) == Some(name)
            && hash.len() == 64
            && hash.chars().all(|character| character.is_ascii_hexdigit())
        {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

fn package_prefix() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "keel-linux-arm64-"
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        "keel-linux-amd64-"
    }
}

fn is_package_name(name: &str) -> bool {
    name.starts_with(package_prefix()) && name.ends_with(".tar.gz")
}

fn write_pending() -> Result<(), String> {
    let path = data_dir().join("updates").join("pending");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|error| format!("could not mark the update as unfinished: {error}"))?;
    file.write_all(b"pending\n")
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(())
}

fn clear_pending() {
    let _ = fs::remove_file(data_dir().join("updates").join("pending"));
}

fn write_result(message: &str) {
    let path = data_dir().join("updates").join("result");
    let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
    let _ = fs::write(path, message);
}

fn clear_result() {
    let _ = fs::remove_file(data_dir().join("updates").join("result"));
}

fn write_update_progress(state: &str, stage: &str, percent: u8, message: &str) {
    let path = data_dir().join("updates").join("progress.json");
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let progress = UpdateProgress {
        state: state.into(),
        stage: stage.into(),
        percent: percent.min(100),
        message: message.into(),
    };
    let Ok(body) = serde_json::to_vec(&progress) else {
        return;
    };
    let temp = path.with_extension("json.tmp");
    if fs::write(&temp, body).is_ok() {
        let _ = fs::rename(temp, path);
    }
}

fn sync_file(path: &Path) -> Result<(), String> {
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn hold_boot_rollback() -> Result<(), String> {
    let status = Command::new("systemctl")
        .args(["mask", "--runtime", "keel-update-rollback.service"])
        .status()
        .map_err(|error| format!("could not hold the restore service: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("could not hold the restore service.".into())
    }
}

fn release_boot_rollback() {
    let _ = Command::new("systemctl")
        .args(["unmask", "--runtime", "keel-update-rollback.service"])
        .status();
}

fn run_rollback_script() -> Result<(), String> {
    let output = Command::new("/usr/lib/keel/rollback-update")
        .arg("--now")
        .output()
        .map_err(|error| format!("could not start the restore program: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            "The restore program did not finish.".into()
        } else {
            detail
        })
    }
}

fn restart_keel() -> Result<(), String> {
    let status = Command::new("systemctl")
        .args(["restart", "keel.service"])
        .status()
        .map_err(|error| format!("could not restart Keel: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Keel did not restart.".into())
    }
}

async fn wait_for_version(expected: &str) -> Result<(), String> {
    let port = std::env::var("KEEL_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(9478);
    let url = format!("http://127.0.0.1:{port}/api/health");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| error.to_string())?;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let Ok(response) = client.get(&url).send().await else {
            continue;
        };
        let Ok(body) = response.json::<serde_json::Value>().await else {
            continue;
        };
        if body.get("ok") == Some(&serde_json::Value::Bool(true))
            && body.get("version").and_then(|value| value.as_str()) == Some(expected)
        {
            return Ok(());
        }
    }
    Err(format!(
        "Keel {expected} did not answer.{}",
        explain_startup_log(&recent_service_log())
    ))
}

fn recent_service_log() -> String {
    let Ok(output) = Command::new("journalctl")
        .args(["-u", "keel.service", "-n", "40", "--no-pager", "-o", "cat"])
        .output()
    else {
        return String::new();
    };
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Turn the service log into one plain sentence. Empty when it does not say why.
fn explain_startup_log(log: &str) -> String {
    let lower = log.to_lowercase();
    if lower.contains("203/exec") || lower.contains("permission denied") {
        return " The new program was saved without permission to run.".into();
    }
    if let Some(line) = log.lines().rev().find(|line| {
        let line = line.to_lowercase();
        line.contains("panic") || line.contains("overflowed its stack") || line.contains("fatal runtime error")
    }) {
        let line = line.trim();
        let line = if line.len() > 180 {
            &line[..180]
        } else {
            line
        };
        return format!(" {line}");
    }
    String::new()
}

fn mark_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("could not read permissions for {}: {error}", path.display()))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        fs::set_permissions(path, permissions).map_err(|error| {
            format!(
                "could not allow {} to run: {error}",
                path.display()
            )
        })?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn github_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .timeout(Duration::from_secs(10 * 60))
        .http1_only()
        .user_agent(format!("keel/{}", env!("CARGO_PKG_VERSION")))
        .default_headers(github_headers()?)
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .map_err(|error| error.to_string())
}

/// Reqwest reports a stopped download as "error decoding response body".
/// The useful reason is the cause underneath that sentence.
fn download_error(error: reqwest::Error) -> String {
    let mut detail = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(inner) = source {
        let text = inner.to_string();
        if !detail.contains(&text) {
            detail.push_str(": ");
            detail.push_str(&text);
        }
        source = inner.source();
    }
    format!("download failed: {detail}")
}

fn github_headers() -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    let token = load_github_token()?;
    if let Some(value) = github_authorization(&token)? {
        headers.insert(AUTHORIZATION, value);
    }
    Ok(headers)
}

fn asset_download_url(asset: &GithubAsset) -> &str {
    if asset.url.starts_with("https://api.github.com/") {
        asset.url.as_str()
    } else {
        asset.browser_download_url.as_str()
    }
}

fn github_token_path() -> PathBuf {
    data_dir().join("github.token")
}

fn load_github_token() -> Result<String, String> {
    if let Ok(token) = std::env::var("KEEL_GITHUB_TOKEN") {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let Ok(bytes) = fs::read(github_token_path()) else {
        return Ok(String::new());
    };
    String::from_utf8(bytes)
        .map(|text| text.trim().to_string())
        .map_err(|_| "The saved GitHub token is not valid text.".into())
}

pub fn github_token_is_set() -> bool {
    load_github_token()
        .map(|token| !token.is_empty())
        .unwrap_or(false)
}

pub fn github_can_connect() -> bool {
    !github_client_id().is_empty()
}

pub fn github_client_id() -> String {
    if let Ok(id) = std::env::var("KEEL_GITHUB_CLIENT_ID") {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return id;
        }
    }
    if let Ok(bytes) = fs::read(data_dir().join("github.client_id")) {
        if let Ok(text) = String::from_utf8(bytes) {
            let id = text.trim().to_string();
            if !id.is_empty() {
                return id;
            }
        }
    }
    BAKED_GITHUB_CLIENT_ID.trim().to_string()
}

#[derive(Deserialize)]
pub struct GithubClientIdRequest {
    pub client_id: String,
}

pub fn save_github_client_id(client_id: &str) -> Result<String, String> {
    let client_id = client_id.trim();
    let path = data_dir().join("github.client_id");
    if client_id.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok("The GitHub app id was removed from this computer.".into());
    }
    if client_id
        .chars()
        .any(|character| !character.is_ascii_alphanumeric() && character != '.' && character != '_')
        || client_id.len() < 8
        || client_id.len() > 64
    {
        return Err("That is not a GitHub app id.".into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&path, format!("{client_id}\n")).map_err(|error| error.to_string())?;
    Ok("GitHub app id saved. Press Connect GitHub.".into())
}

#[derive(Deserialize)]
pub struct GithubTokenRequest {
    pub token: String,
}

pub fn save_github_token(token: &str) -> Result<String, String> {
    let token = token.trim();
    let path = github_token_path();
    if token.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok("The GitHub token was removed from this computer.".into());
    }
    github_authorization(token)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = path.with_extension("token.tmp");
    fs::write(&tmp, format!("{token}\n")).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(tmp, path).map_err(|error| error.to_string())?;
    Ok("GitHub token saved on this computer. Check for an update on this page.".into())
}

fn github_authorization(token: &str) -> Result<Option<HeaderValue>, String> {
    if token.is_empty() {
        return Ok(None);
    }
    if token
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("KEEL_GITHUB_TOKEN contains a space or control character.".into());
    }
    let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
        .map_err(|_| "KEEL_GITHUB_TOKEN is not a valid header.".to_string())?;
    value.set_sensitive(true);
    Ok(Some(value))
}

fn allowed_download_url(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    matches!(
        host,
        "github.com"
            | "api.github.com"
            | "release-assets.github.com"
            | "objects.githubusercontent.com"
            | "github-releases.githubusercontent.com"
            | "githubusercontent.com"
    ) || host.ends_with(".githubusercontent.com")
}

fn unavailable(current: &'static str, can_restore: bool, message: String) -> UpdateStatus {
    UpdateStatus {
        state: "unavailable",
        current,
        latest: None,
        update_available: false,
        release_url: None,
        published_at: None,
        appliance_asset: None,
        checksum_asset: None,
        signature: "unchecked",
        can_apply: false,
        can_restore,
        github_token_set: github_token_is_set(),
        github_can_connect: github_can_connect(),
        message,
    }
}

fn newer_than(candidate: &str, current: &str) -> bool {
    version_parts(candidate) > version_parts(current)
}

fn version_parts(value: &str) -> [u64; 3] {
    let mut parts = [0; 3];
    for (index, part) in value.split('.').take(3).enumerate() {
        parts[index] = part
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
    }
    parts
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    if text.len() % 2 != 0 || !text.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err("The signature is not valid hex.".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&text[index..index + 2], 16).map_err(|error| error.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tar::Builder;

    const RFC8032_TEST_PUBLIC: &str =
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

    #[test]
    fn github_token_must_be_a_single_word() {
        assert!(github_authorization("").unwrap().is_none());
        assert!(github_authorization("keel_token").unwrap().is_some());
        assert!(github_authorization("bad token").is_err());
    }

    #[test]
    fn saves_github_token_without_echoing_it() {
        let dir = std::env::temp_dir().join(format!("keel-gh-token-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::env::remove_var("KEEL_GITHUB_TOKEN");
        std::env::remove_var("KEEL_GITHUB_CLIENT_ID");
        std::env::set_var("KEEL_DATA", &dir);
        let message = super::save_github_token("ghp_testtoken").unwrap();
        assert!(message.contains("saved"));
        assert!(!message.contains("ghp_testtoken"));
        assert!(super::github_token_is_set());
        assert_eq!(super::load_github_token().unwrap(), "ghp_testtoken");
        super::save_github_token("").unwrap();
        assert!(!super::github_token_is_set());
        let id_message = super::save_github_client_id("Iv1.keeltestid").unwrap();
        assert!(id_message.contains("saved"));
        assert!(super::github_can_connect());
        assert_eq!(super::github_client_id(), "Iv1.keeltestid");
        super::save_github_client_id("").unwrap();
        assert!(!super::github_can_connect());
        std::env::remove_var("KEEL_DATA");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_name_matches_this_computer() {
        let ours = format!("{}v0.1.25.tar.gz", package_prefix());
        assert!(is_package_name(&ours));
        let other = if package_prefix().contains("arm64") {
            "keel-linux-amd64-v0.1.25.tar.gz"
        } else {
            "keel-linux-arm64-v0.1.25.tar.gz"
        };
        assert!(!is_package_name(other));
        assert!(!is_package_name("notes.txt"));
    }

    #[test]
    fn compares_release_versions() {
        assert!(newer_than("0.2.0", "0.1.9"));
        assert!(newer_than("1.0.0", "0.99.99"));
        assert!(!newer_than("0.1.0", "0.1.0"));
        assert!(!newer_than("0.0.9", "0.1.0"));
    }

    #[test]
    fn update_progress_round_trips_without_secrets() {
        let progress = UpdateProgress {
            state: "running".into(),
            stage: "downloading".into(),
            percent: 42,
            message: "Downloading the Keel update.".into(),
        };
        let encoded = serde_json::to_string(&progress).unwrap();
        let decoded: UpdateProgress = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, progress);
        assert!(!encoded.contains("token"));
    }

    #[test]
    fn downloads_private_release_files_from_the_github_api() {
        let asset = GithubAsset {
            name: "SHA256SUMS".into(),
            url: "https://api.github.com/repos/robbykube/keel/releases/assets/1".into(),
            browser_download_url: "https://github.com/robbykube/keel/releases/download/v0.1.2/SHA256SUMS".into(),
        };
        assert_eq!(
            asset_download_url(&asset),
            "https://api.github.com/repos/robbykube/keel/releases/assets/1"
        );
    }

    #[test]
    fn release_key_is_not_the_public_test_vector() {
        let key = release_public_key().expect("public key");
        assert_ne!(encode_hex(key.as_bytes()), RFC8032_TEST_PUBLIC);
    }

    #[test]
    fn rejects_a_bad_signature() {
        let secret = [7u8; 32];
        let key = SigningKey::from_bytes(&secret);
        let message = b"SHA256SUMS";
        let signature = key.sign(message);
        let public = key.verifying_key();
        assert!(public.verify(message, &signature).is_ok());
        assert!(public.verify(b"other", &signature).is_err());
    }

    #[test]
    fn reads_the_package_checksum() {
        let sums = "abc123  other.txt\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  keel-linux-amd64-v0.1.1.tar.gz\n";
        assert_eq!(
            checksum_for(sums, "keel-linux-amd64-v0.1.1.tar.gz").as_deref(),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
        assert!(checksum_for(sums, "missing.tar.gz").is_none());
    }

    #[test]
    fn package_paths_cannot_escape() {
        assert!(safe_member(Path::new("keel/keel")).is_ok());
        assert!(safe_member(Path::new("keel/web/index.html")).is_ok());
        assert!(safe_member(Path::new("keel/../etc/passwd")).is_err());
        assert!(safe_member(Path::new("/etc/passwd")).is_err());
        assert!(safe_member(Path::new("keel/etc/shadow")).is_err());
    }

    #[test]
    fn archive_rejects_links_and_keeps_the_program() {
        let bytes = sample_package();
        let dir = std::env::temp_dir().join(format!("keel-pkg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        extract_package(&bytes, &dir).unwrap();
        assert!(dir.join("keel").is_file());
        assert!(dir.join("web").join("index.html").is_file());
        assert!(!dir.join("keel.service").exists() || dir.join("keel.service").is_file());
        let _ = fs::remove_dir_all(&dir);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut builder = Builder::new(&mut encoder);
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, "keel/keel", std::io::empty())
                .unwrap();
            builder.finish().unwrap();
        }
        let bad = encoder.finish().unwrap();
        assert!(extract_package(&bad, &dir).is_err());
    }

    #[test]
    fn replace_file_keeps_a_readable_program() {
        let dir = std::env::temp_dir().join(format!("keel-replace-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source");
        let dest = dir.join("keel");
        fs::write(&dest, b"old").unwrap();
        fs::write(&source, b"new").unwrap();
        replace_file(&source, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&dest).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_program_without_permission_to_run_is_explained() {
        let log = "keel.service: Main process exited, code=exited, status=203/EXEC";
        assert!(super::explain_startup_log(log).contains("permission to run"));
        assert!(super::explain_startup_log("listening").is_empty());
    }

    #[test]
    fn signed_update_is_offered_only_on_an_installed_machine() {
        let ready = HostCaps {
            linux: true,
            live: false,
            helper: true,
        };
        let decision = decide("0.1.1", "0.1.2", true, "verified", &ready);
        assert!(decision.can_apply);
        assert!(decision.message.contains("signature checks out"));

        let windows = HostCaps {
            linux: false,
            live: false,
            helper: false,
        };
        assert!(!decide("0.1.1", "0.1.2", true, "verified", &windows).can_apply);
        assert!(!decide("0.1.1", "0.1.2", true, "missing", &ready).can_apply);
        assert!(!decide("0.1.1", "0.1.2", true, "invalid", &ready).can_apply);
        let download = decide("0.1.1", "0.1.2", true, "download_failed", &ready);
        assert!(!download.can_apply);
        assert!(download.message.contains("could not download"));
        let current = decide("0.1.2", "0.1.2", false, "unchecked", &ready);
        assert!(!current.can_apply);
        assert!(current.message.contains("GitHub release 0.1.2"));
    }

    fn sample_package() -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut builder = Builder::new(&mut encoder);
            append(&mut builder, "keel/keel", b"#!/bin/keel");
            append(&mut builder, "keel/web/index.html", b"<p>keel</p>");
            builder.finish().unwrap();
        }
        encoder.finish().unwrap()
    }

    fn append(builder: &mut Builder<&mut GzEncoder<Vec<u8>>>, path: &str, bytes: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, bytes).unwrap();
    }
}
