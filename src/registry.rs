use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::{project_key, registry_file_path};

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredProject {
    pub project_key: String,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub config_path: PathBuf,
    pub last_seen_ts: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectRegistry {
    pub projects: HashMap<String, RegisteredProject>,
}

impl ProjectRegistry {
    pub fn load() -> Self {
        let path = registry_file_path();
        if !path.is_file() {
            return Self::default();
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = registry_file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn register(config_path: &Path) -> Result<()> {
        let abs_config = fs::canonicalize(config_path)?;
        let key = project_key(&abs_config)?;

        let project_dir = abs_config
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| abs_config.clone());

        let project_name = project_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        let mut registry = Self::load();
        registry.projects.insert(
            key.clone(),
            RegisteredProject {
                project_key: key,
                project_name,
                project_dir,
                config_path: abs_config,
                last_seen_ts: current_timestamp(),
            },
        );

        registry.save()
    }

    pub fn unregister(project_key: &str) -> Result<bool> {
        let mut registry = Self::load();
        if registry.projects.remove(project_key).is_some() {
            registry.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_all(&self) -> Vec<RegisteredProject> {
        let mut list: Vec<RegisteredProject> = self.projects.values().cloned().collect();
        list.sort_by(|a, b| a.project_key.cmp(&b.project_key));
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_registry_serialization() {
        let mut reg = ProjectRegistry::default();
        reg.projects.insert(
            "test-app-12345678".to_string(),
            RegisteredProject {
                project_key: "test-app-12345678".to_string(),
                project_name: "test-app".to_string(),
                project_dir: PathBuf::from("/tmp/test-app"),
                config_path: PathBuf::from("/tmp/test-app/procman.yaml"),
                last_seen_ts: 1700000000,
            },
        );

        let json = serde_json::to_string(&reg).unwrap();
        let loaded: ProjectRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(
            loaded.projects.get("test-app-12345678").unwrap().project_name,
            "test-app"
        );
    }
}
