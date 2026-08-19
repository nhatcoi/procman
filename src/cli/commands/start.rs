use anyhow::Result;

use crate::engine::config::require_config;
use crate::engine::supervisor;

pub fn execute(name: Option<String>, forward: bool) -> Result<()> {
    let (config_path, config) = require_config()?;
    supervisor::start(&config_path, &config, name.as_deref(), forward)
}
