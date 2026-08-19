use anyhow::Result;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::paths::state_file_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcEntry {
    pub pid: i32,
    pub cmd: String,
    pub cwd: String,
    #[serde(default)]
    pub port: Option<u16>,
    pub started_at: String,
    #[serde(default)]
    pub log_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelEntry {
    pub pid: i32,
    pub port: u16,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub log_file: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub processes: HashMap<String, ProcEntry>,
    #[serde(default)]
    pub tunnels: HashMap<String, TunnelEntry>,
}

pub fn is_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    kill(Pid::from_raw(pid), None).is_ok()
}

pub fn read_state(config_path: &Path) -> State {
    let Ok(path) = state_file_path(config_path) else {
        return State::default();
    };
    if !path.is_file() {
        return State::default();
    }
    let Ok(content) = fs::read_to_string(&path) else {
        return State::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn write_state(config_path: &Path, state: &State) -> Result<()> {
    let path = state_file_path(config_path)?;
    let content = serde_json::to_string_pretty(state)?;
    fs::write(&path, content)?;
    Ok(())
}
