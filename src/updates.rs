use serde::{Deserialize, Serialize};
use std::time::Duration;

const REPOSITORY: &str = "robbykube/keel";

#[derive(Serialize)]
pub struct UpdateStatus {
    pub state: &'static str,
    pub current: &'static str,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub published_at: Option<String>,
    pub message: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
}

pub async fn check() -> UpdateStatus {
    let current = env!("CARGO_PKG_VERSION");
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(format!("keel/{current}"))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return unavailable(
                current,
                format!("could not prepare the update check: {error}"),
            )
        }
    };

    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => return unavailable(current, format!("GitHub could not be reached: {error}")),
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return unavailable(current, "No published Keel release exists yet.".into());
    }
    if !response.status().is_success() {
        return unavailable(
            current,
            format!("GitHub returned HTTP {}.", response.status().as_u16()),
        );
    }

    let release = match response.json::<GithubRelease>().await {
        Ok(release) => release,
        Err(error) => {
            return unavailable(
                current,
                format!("GitHub returned an unreadable release: {error}"),
            )
        }
    };
    if release.draft || release.prerelease {
        return unavailable(
            current,
            "The latest GitHub release is not a stable release.".into(),
        );
    }

    let latest = release.tag_name.trim_start_matches('v').to_string();
    let update_available = newer_than(&latest, current);
    UpdateStatus {
        state: "checked",
        current,
        latest: Some(latest.clone()),
        update_available,
        release_url: Some(release.html_url),
        published_at: release.published_at,
        message: if update_available {
            format!("Keel {latest} is available. Installation is not automatic yet.")
        } else {
            "This Keel instance is up to date.".into()
        },
    }
}

fn unavailable(current: &'static str, message: String) -> UpdateStatus {
    UpdateStatus {
        state: "unavailable",
        current,
        latest: None,
        update_available: false,
        release_url: None,
        published_at: None,
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
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::newer_than;

    #[test]
    fn compares_release_versions() {
        assert!(newer_than("0.2.0", "0.1.9"));
        assert!(newer_than("1.0.0", "0.99.99"));
        assert!(!newer_than("0.1.0", "0.1.0"));
        assert!(!newer_than("0.0.9", "0.1.0"));
    }
}
