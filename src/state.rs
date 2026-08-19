use anyhow::{Context, Result};
use nix::sys::signal::kill;
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::paths::state_file_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcEntry {
    pub pid: i32,
    pub cmd: String,
    pub cwd: String,
    pub log_file: String,
    pub port: Option<u16>,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelEntry {
    pub pid: i32,
    pub port: u16,
    pub url: Option<String>,
    pub log_file: String,
    pub started_at: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub processes: HashMap<String, ProcEntry>,
    #[serde(default)]
    pub tunnels: HashMap<String, TunnelEntry>,
}

pub fn read_state(config_path: &Path) -> State {
    let file = match state_file_path(config_path) {
        Ok(f) => f,
        Err(_) => return State::default(),
    };

    if !file.is_file() {
        return State::default();
    }

    match fs::read_to_string(&file) {
        Ok(content) => serde_json::from_str::<State>(&content).unwrap_or_default(),
        Err(_) => State::default(),
    }
}

pub fn write_state(config_path: &Path, state: &State) -> Result<()> {
    let file = state_file_path(config_path)?;
    let content = serde_json::to_string_pretty(state)
        .context("Failed to serialize state to JSON")?;
    fs::write(&file, content)
        .with_context(|| format!("Failed to write state file at {:?}", file))?;
    Ok(())
}

pub fn is_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    kill(Pid::from_raw(pid), None).is_ok()
}
