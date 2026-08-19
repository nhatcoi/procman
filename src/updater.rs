use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::update_cache_file_path;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "nhatcoi/procman";
const CACHE_TTL_SECONDS: u64 = 86400; // 24 hours

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateCache {
    pub last_checked_ts: u64,
    pub latest_version: Option<String>,
}

impl UpdateCache {
    pub fn load() -> Self {
        let path = update_cache_file_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cache) = serde_json::from_str::<UpdateCache>(&content) {
                return cache;
            }
        }
        UpdateCache::default()
    }

    pub fn save(&self) {
        let path = update_cache_file_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn is_stale(&self, ttl: u64) -> bool {
        let now = now_ts();
        now.saturating_sub(self.last_checked_ts) > ttl
    }
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn parse_version_tuple(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches('v');
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = if parts.len() >= 3 {
        let num_str: String = parts[2].chars().take_while(|c| c.is_ascii_digit()).collect();
        num_str.parse::<u64>().unwrap_or(0)
    } else {
        0
    };
    Some((major, minor, patch))
}

pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (parse_version_tuple(current), parse_version_tuple(candidate)) {
        (Some(cur), Some(cand)) => cand > cur,
        _ => false,
    }
}

pub fn fetch_remote_latest_version() -> Option<String> {
    // 1. Try GitHub Releases API
    let gh_api_url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "-H",
            "User-Agent: procman-cli",
            &gh_api_url,
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(tag) = json_val.get("tag_name").and_then(|v| v.as_str()) {
                    let clean = tag.trim().trim_start_matches('v').to_string();
                    if !clean.is_empty() {
                        return Some(clean);
                    }
                }
            }
        }
    }

    // 2. Fallback: Check raw Cargo.toml on main branch
    let raw_cargo_url = format!(
        "https://raw.githubusercontent.com/{}/main/Cargo.toml",
        GITHUB_REPO
    );
    let cargo_output = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            &raw_cargo_url,
        ])
        .output();

    if let Ok(out) = cargo_output {
        if out.status.success() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("version") {
                        if let Some((_, val)) = trimmed.split_once('=') {
                            let clean = val.trim().trim_matches('"').trim().to_string();
                            if !clean.is_empty() {
                                return Some(clean);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn check_latest_version(force: bool) -> Option<String> {
    let mut cache = UpdateCache::load();

    if !force && !cache.is_stale(CACHE_TTL_SECONDS) {
        return cache.latest_version.clone();
    }

    if let Some(remote_ver) = fetch_remote_latest_version() {
        cache.last_checked_ts = now_ts();
        cache.latest_version = Some(remote_ver.clone());
        cache.save();
        Some(remote_ver)
    } else {
        // Update check timestamp even on failure to avoid continuous timeouts
        cache.last_checked_ts = now_ts();
        cache.save();
        cache.latest_version
    }
}

/// Spawns a non-blocking background thread to refresh the update cache if stale.
pub fn trigger_background_update_check() {
    let cache = UpdateCache::load();
    if cache.is_stale(CACHE_TTL_SECONDS) {
        std::thread::spawn(|| {
            let _ = check_latest_version(true);
        });
    }
}

/// Returns an update notice banner string if a newer version is cached and available.
pub fn get_update_notice() -> Option<String> {
    let cache = UpdateCache::load();
    if let Some(latest) = cache.latest_version {
        if is_newer(CURRENT_VERSION, &latest) {
            return Some(format_update_banner(CURRENT_VERSION, &latest));
        }
    }
    None
}

pub fn format_update_banner(current: &str, latest: &str) -> String {
    format!(
        "\n╭─────────────────────────────────────────────────────────────╮\n\
         │  🔔 Update available: v{} ➔ v{}                       │\n\
         │  Run `procman upgrade` to update to the latest release.    │\n\
         ╰─────────────────────────────────────────────────────────────╯\n",
        current, latest
    )
}

/// Interactive upgrade process
pub fn run_upgrade(yes: bool) -> Result<()> {
    println!("🔍 Checking for procman updates...");

    let latest = match check_latest_version(true) {
        Some(v) => v,
        None => {
            println!("⚠️  Could not reach GitHub to check for updates. Please check your internet connection.");
            return Ok(());
        }
    };

    if !is_newer(CURRENT_VERSION, &latest) {
        println!(
            "✅ You are already using the latest version of procman (v{}).",
            CURRENT_VERSION
        );
        return Ok(());
    }

    println!();
    println!(
        "🚀 A new version of procman is available: v{} ➔ v{}",
        CURRENT_VERSION, latest
    );

    if !yes {
        print!("   Would you like to upgrade now? [y/N]: ");
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read user response")?;

        let ans = input.trim().to_lowercase();
        if ans != "y" && ans != "yes" {
            println!("Upgrade skipped.");
            return Ok(());
        }
    }

    println!("\n📦 Downloading and installing procman v{}...", latest);

    // Try cargo install --git first
    let has_cargo = Command::new("cargo")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let success = if has_cargo {
        println!("⚙️  Compiling via Cargo...");
        let status = Command::new("cargo")
            .args([
                "install",
                "--git",
                &format!("https://github.com/{}.git", GITHUB_REPO),
                "--force",
            ])
            .status();
        status.map(|s| s.success()).unwrap_or(false)
    } else {
        println!("⚙️  Running quick install script...");
        let status = Command::new("sh")
            .arg("-c")
            .arg(&format!(
                "curl -fsSL https://raw.githubusercontent.com/{}/main/install.sh | sh",
                GITHUB_REPO
            ))
            .status();
        status.map(|s| s.success()).unwrap_or(false)
    };

    if success {
        // Refresh local cache to reflect current updated version
        let mut cache = UpdateCache::load();
        cache.last_checked_ts = now_ts();
        cache.latest_version = Some(latest.clone());
        cache.save();

        println!("\n🎉 Successfully updated procman to v{}!", latest);
        println!("   Run `procman --version` to verify.");
    } else {
        eprintln!(
            "\n❌ Upgrade failed. You can manually run:\n   cargo install --git https://github.com/{}.git --force\n   or\n   curl -fsSL https://raw.githubusercontent.com/{}/main/install.sh | sh",
            GITHUB_REPO, GITHUB_REPO
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing_and_comparison() {
        assert!(is_newer("0.2.0", "0.2.1"));
        assert!(is_newer("0.2.0", "0.3.0"));
        assert!(is_newer("0.2.0", "1.0.0"));
        assert!(is_newer("0.2.0", "v0.2.1"));
        assert!(is_newer("v0.2.0", "0.3.0"));

        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.3.0", "0.2.0"));
        assert!(!is_newer("1.0.0", "0.9.9"));

        assert_eq!(parse_version_tuple("0.2.0"), Some((0, 2, 0)));
        assert_eq!(parse_version_tuple("v1.5.12"), Some((1, 5, 12)));
        assert_eq!(parse_version_tuple("v0.3.0-rc1"), Some((0, 3, 0)));
    }

    #[test]
    fn test_format_update_banner() {
        let banner = format_update_banner("0.2.0", "0.3.0");
        assert!(banner.contains("0.2.0 ➔ v0.3.0"));
        assert!(banner.contains("procman upgrade"));
    }
}
