use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_CANDIDATE_FILENAMES: &[&str] = &[
    "procman.yaml",
    "procman.yml",
    "procman.json",
    ".procman.yaml",
    ".procman.yml",
    ".procman.json",
];

const JSON_EXTENSION: &str = ".json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDef {
    pub cmd: String,
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub port: Option<u16>,
    pub forward: Option<bool>,
    pub tunnel: Option<bool>,
    pub free_port: Option<bool>,
    pub kill_before_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub name: Option<String>,
    pub processes: HashMap<String, ProcessDef>,
}

pub fn find_config_path(start_dir: Option<&Path>) -> Option<PathBuf> {
    let current_dir = match start_dir {
        Some(d) => d.to_path_buf(),
        None => env::current_dir().ok()?,
    };

    let home = env::var("HOME").ok().map(PathBuf::from);
    let mut dir = fs::canonicalize(&current_dir).unwrap_or(current_dir);

    loop {
        for candidate in CONFIG_CANDIDATE_FILENAMES {
            let path = dir.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }

        if let Some(ref h) = home {
            if dir == *h {
                break;
            }
        }

        match dir.parent() {
            Some(parent) => {
                if parent == dir {
                    break;
                }
                dir = parent.to_path_buf();
            }
            None => break,
        }
    }

    None
}

pub fn load_config(config_path: &Path) -> Result<Config> {
    let raw = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file at {:?}", config_path))?;

    let is_json = config_path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or_else(|| config_path.to_string_lossy().ends_with(JSON_EXTENSION));

    let config: Config = if is_json {
        serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse JSON config at {:?}", config_path))?
    } else {
        serde_yaml::from_str(&raw)
            .with_context(|| format!("Failed to parse YAML config at {:?}", config_path))?
    };

    if config.processes.is_empty() {
        return Err(anyhow!(
            "Invalid config at {:?}: \"processes\" map is empty",
            config_path
        ));
    }

    Ok(config)
}

pub fn require_config() -> Result<(PathBuf, Config)> {
    let config_path = find_config_path(None).ok_or_else(|| {
        anyhow!("No procman config found (looked for procman.yaml / procman.json walking up from cwd).")
    })?;

    let config = load_config(&config_path)?;
    Ok((config_path, config))
}
