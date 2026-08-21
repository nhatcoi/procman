use anyhow::Result;
use std::path::PathBuf;

use crate::engine::mcp;

pub fn execute(dir: Option<String>) -> Result<()> {
    let path = dir.map(|d| {
        PathBuf::from(&d)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&d))
    });

    mcp::run_stdio_server(path.as_deref())
}
