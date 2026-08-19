use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILENAMES: &[&str] = &[
    "procman.yaml",
    "procman.yml",
    "procman.json",
    ".procman.yaml",
    ".procman.yml",
    ".procman.json",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WatchConfig {
    Enabled(bool),
    Paths(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDef {
    pub cmd: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub forward: bool,
    #[serde(default)]
    pub tunnel: bool,
    #[serde(default)]
    pub free_port: bool,
    #[serde(default)]
    pub log_file: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub watch: Option<WatchConfig>,
    #[serde(default)]
    pub watch_ignore: Vec<String>,
}

impl ProcessDef {
    pub fn is_watch_enabled(&self) -> bool {
        match &self.watch {
            Some(WatchConfig::Enabled(b)) => *b,
            Some(WatchConfig::Paths(paths)) => !paths.is_empty(),
            None => false,
        }
    }

    pub fn get_watch_paths(&self) -> Vec<String> {
        match &self.watch {
            Some(WatchConfig::Enabled(true)) => vec![".".to_string()],
            Some(WatchConfig::Paths(paths)) => paths.clone(),
            _ => vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub processes: HashMap<String, ProcessDef>,
}

pub fn find_config_path(start_dir: Option<&Path>) -> Option<PathBuf> {
    let mut current = match start_dir {
        Some(p) => p.to_path_buf(),
        None => env::current_dir().ok()?,
    };

    loop {
        for filename in CONFIG_FILENAMES {
            let candidate = current.join(filename);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}

pub fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file at {:?}", path))?;

    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_lowercase();

    if filename.ends_with(".json") {
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON config at {:?}", path))
    } else {
        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML config at {:?}", path))
    }
}

pub fn require_config() -> Result<(PathBuf, Config)> {
    let config_path = find_config_path(None).ok_or_else(|| {
        anyhow!(
            "No procman config found (looked for procman.yaml / procman.json walking up from cwd)."
        )
    })?;

    let config = load_config(&config_path)?;
    Ok((config_path, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_config_deserialization_bool() {
        let yaml = r#"
processes:
  api:
    cmd: "cargo run"
    watch: true
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let api = config.processes.get("api").unwrap();
        assert!(api.is_watch_enabled());
        assert_eq!(api.get_watch_paths(), vec!["."]);
    }

    #[test]
    fn test_watch_config_deserialization_paths() {
        let yaml = r#"
processes:
  api:
    cmd: "cargo run"
    watch:
      - "src"
      - "Cargo.toml"
    watch_ignore:
      - "target"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let api = config.processes.get("api").unwrap();
        assert!(api.is_watch_enabled());
        assert_eq!(api.get_watch_paths(), vec!["src", "Cargo.toml"]);
        assert_eq!(api.watch_ignore, vec!["target"]);
    }
}
