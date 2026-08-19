use anyhow::Result;

use crate::engine::config::require_config;
use crate::engine::supervisor;
use crate::engine::watcher;

pub fn execute(name: Option<String>) -> Result<()> {
    let (config_path, config) = require_config()?;

    // First ensure the processes are running
    supervisor::start(&config_path, &config, name.as_deref(), false)?;

    // Then start the file watcher loop
    let targets = name.map(|n| vec![n]);
    watcher::watch_and_reload(&config_path, &config, targets)
}
