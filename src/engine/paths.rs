use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const APP_NAME: &str = "procman";
const STATE_FILENAME: &str = "state.json";
const LOGS_DIRNAME: &str = "logs";
const HASH_KEY_LENGTH: usize = 8;
const FALLBACK_DIR: &str = "/tmp";

fn xdg_state_home() -> PathBuf {
    if let Ok(val) = env::var("XDG_STATE_HOME") {
        if !val.is_empty() {
            return PathBuf::from(val);
        }
    }
    dirs::home_dir()
        .map(|h| h.join(".local").join("state"))
        .unwrap_or_else(|| PathBuf::from(FALLBACK_DIR))
}

fn xdg_data_home() -> PathBuf {
    if let Ok(val) = env::var("XDG_DATA_HOME") {
        if !val.is_empty() {
            return PathBuf::from(val);
        }
    }
    dirs::home_dir()
        .map(|h| h.join(".local").join("share"))
        .unwrap_or_else(|| PathBuf::from(FALLBACK_DIR))
}

pub fn project_key(config_path: &Path) -> Result<String> {
    let abs = fs::canonicalize(config_path)
        .with_context(|| format!("Failed to canonicalize config path {:?}", config_path))?;

    let parent = abs.parent().unwrap_or(&abs);
    let base = parent
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "root".to_string());

    let mut hasher = Sha1::new();
    hasher.update(abs.to_string_lossy().as_bytes());
    let hash_hex = hex::encode(hasher.finalize());
    let short_hash = &hash_hex[..HASH_KEY_LENGTH];

    Ok(format!("{}-{}", base, short_hash))
}

pub fn project_state_dir(config_path: &Path) -> Result<PathBuf> {
    let key = project_key(config_path)?;
    let dir = xdg_state_home().join(APP_NAME).join(key);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create state directory at {:?}", dir))?;
    Ok(dir)
}

pub fn project_log_dir(config_path: &Path) -> Result<PathBuf> {
    let key = project_key(config_path)?;
    let dir = xdg_data_home().join(APP_NAME).join(key).join(LOGS_DIRNAME);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create log directory at {:?}", dir))?;
    Ok(dir)
}

pub fn state_file_path(config_path: &Path) -> Result<PathBuf> {
    Ok(project_state_dir(config_path)?.join(STATE_FILENAME))
}

pub fn global_state_root() -> PathBuf {
    xdg_state_home().join(APP_NAME)
}

pub fn global_data_root() -> PathBuf {
    xdg_data_home().join(APP_NAME)
}

pub fn update_cache_file_path() -> PathBuf {
    let dir = global_state_root();
    let _ = fs::create_dir_all(&dir);
    dir.join("update_cache.json")
}

pub fn registry_file_path() -> PathBuf {
    let dir = global_state_root();
    let _ = fs::create_dir_all(&dir);
    dir.join("registry.json")
}
