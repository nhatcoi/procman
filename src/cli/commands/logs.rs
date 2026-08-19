use anyhow::Result;

use crate::engine::config::require_config;
use crate::engine::logs::stream_logs;
use crate::engine::supervisor::log_file_for;

pub fn execute(name: String, follow: bool, lines: usize) -> Result<()> {
    let (config_path, _) = require_config()?;
    let path = log_file_for(&config_path, &name)?;
    stream_logs(&path, lines, follow)
}
