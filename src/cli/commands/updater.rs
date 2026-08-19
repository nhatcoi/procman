use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::paths::update_cache_file_path;

const GITHUB_RELEASES_API: &str = "https://api.github.com/repos/nhatcoi/procman/releases/latest";
const CACHE_TTL_SECS: u64 = 86400; // 24 hours
const CARGO_GIT_URL: &str = "https://github.com/nhatcoi/procman.git";

#[derive(Debug, Serialize, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    name: Option<String>,
    html_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCache {
    pub last_checked_ts: u64,
    pub latest_version: String,
    pub release_url: Option<String>,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_cache() -> Option<UpdateCache> {
    let path = update_cache_file_path();
    if !path.is_file() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_cache(cache: &UpdateCache) {
    let path = update_cache_file_path();
    if let Ok(content) = serde_json::to_string(cache) {
        let _ = fs::write(path, content);
    }
}

pub fn fetch_latest_release() -> Result<Option<UpdateCache>> {
    let output = Command::new("curl")
        .args([
            "-sL",
            "--max-time",
            "5",
            "-H",
            "User-Agent: procman-cli-updater",
            "-H",
            "Accept: application/vnd.github.v3+json",
            GITHUB_RELEASES_API,
        ])
        .output()
        .context("Failed to run curl to check GitHub releases")?;

    if !output.status.success() {
        return Ok(None);
    }

    let body = String::from_utf8_lossy(&output.stdout);
    if let Ok(release) = serde_json::from_str::<GithubReleaseResponse>(&body) {
        let clean_version = release.tag_name.trim_start_matches('v').to_string();
        let cache = UpdateCache {
            last_checked_ts: current_timestamp(),
            latest_version: clean_version,
            release_url: release.html_url,
        };
        write_cache(&cache);
        Ok(Some(cache))
    } else {
        Ok(None)
    }
}

pub fn check_for_updates_background() {
    let now = current_timestamp();
    if let Some(cache) = read_cache() {
        if now.saturating_sub(cache.last_checked_ts) < CACHE_TTL_SECS {
            return;
        }
    }

    thread::spawn(|| {
        let _ = fetch_latest_release();
    });
}

pub fn parse_semver(v: &str) -> (u64, u64, u64) {
    let parts: Vec<&str> = v.trim_start_matches('v').split('.').collect();
    let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    parse_semver(latest) > parse_semver(current)
}

pub fn get_cached_update_banner() -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    let cache = read_cache()?;

    if is_newer(current, &cache.latest_version) {
        Some(format_update_banner(current, &cache.latest_version))
    } else {
        None
    }
}

pub fn format_update_banner(current: &str, latest: &str) -> String {
    format!(
        "\n╭─────────────────────────────────────────────────────────────╮\n\
         │  🚀 A new release of procman is available: v{} -> v{}  │\n\
         │  Run `procman upgrade` to update to the latest version.     │\n\
         ╰─────────────────────────────────────────────────────────────╯\n",
        current, latest
    )
}

pub fn execute(target_tag: Option<String>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("🔍 Checking for procman updates...");

    let (latest_version, tag_to_install) = if let Some(ref tag) = target_tag {
        let clean = tag.trim_start_matches('v').to_string();
        (clean, tag.clone())
    } else {
        let latest = fetch_latest_release()?
            .map(|c| c.latest_version)
            .unwrap_or_else(|| current_version.to_string());
        let tag = format!("v{}", latest);
        (latest, tag)
    };

    if !is_newer(current_version, &latest_version) && target_tag.is_none() {
        println!(
            "✨ You are already on the latest version of procman (v{})!",
            current_version
        );
        return Ok(());
    }

    println!(
        "📦 Upgrading procman from v{} to tag {} via cargo...",
        current_version, tag_to_install
    );

    let status = Command::new("cargo")
        .args([
            "install",
            "--git",
            CARGO_GIT_URL,
            "--tag",
            &tag_to_install,
            "--force",
        ])
        .status()
        .context("Failed to run `cargo install`. Ensure cargo is available on PATH.")?;

    if status.success() {
        println!(
            "\n🎉 Successfully upgraded procman to version {}!",
            latest_version
        );
    } else {
        println!("\n⚠️  `cargo install --tag` failed. Attempting install from main branch...");
        let fallback_status = Command::new("cargo")
            .args(["install", "--git", CARGO_GIT_URL, "--force"])
            .status()
            .context("Failed to run fallback `cargo install`")?;

        if fallback_status.success() {
            println!("\n🎉 Successfully upgraded procman from GitHub main!");
        } else {
            return Err(anyhow::anyhow!(
                "Failed to upgrade procman via cargo. Check permissions or network connectivity."
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing_and_comparison() {
        assert_eq!(parse_semver("0.1.0"), (0, 1, 0));
        assert_eq!(parse_semver("v0.1.1"), (0, 1, 1));
        assert_eq!(parse_semver("1.2.34"), (1, 2, 34));

        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.0", "1.0.0"));
        assert!(!is_newer("0.1.1", "0.1.1"));
        assert!(!is_newer("0.2.0", "0.1.9"));
    }

    #[test]
    fn test_format_update_banner() {
        let banner = format_update_banner("0.1.0", "0.1.1");
        assert!(banner.contains("v0.1.0 -> v0.1.1"));
        assert!(banner.contains("procman upgrade"));
    }
}
