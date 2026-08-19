use anyhow::Result;

use crate::engine::config::require_config;
use crate::engine::supervisor;

pub fn execute_stop(name: Option<String>) -> Result<()> {
    let (config_path, config) = require_config()?;
    supervisor::stop(&config_path, &config, name.as_deref())
}

pub fn execute_kill(name: Option<String>) -> Result<()> {
    let (config_path, config) = require_config()?;
    supervisor::force_stop(&config_path, &config, name.as_deref())
}

pub fn execute_restart(name: Option<String>, forward: bool) -> Result<()> {
    let (config_path, config) = require_config()?;
    supervisor::restart(&config_path, &config, name.as_deref(), forward)
}
