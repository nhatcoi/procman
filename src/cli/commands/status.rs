use anyhow::Result;

use crate::cli::format::{print_global_processes, print_status_table};
use crate::engine::config::find_config_path;
use crate::engine::config::load_config;
use crate::engine::supervisor;
use crate::tunnels::cloudflare::{forward_start, forward_stop};

pub fn execute_status(name: Option<String>) -> Result<()> {
    if let Some(config_path) = find_config_path(None) {
        if let Ok(config) = load_config(&config_path) {
            let rows = supervisor::status(&config_path, &config, name.as_deref())?;
            print_status_table(&rows);
            return Ok(());
        }
    }

    // Auto-fallback: if outside a repo, scan and display all running processes across all projects
    let rows = supervisor::scan_global_processes()?;
    print_global_processes(&rows);
    Ok(())
}

pub fn execute_ps() -> Result<()> {
    let rows = supervisor::scan_global_processes()?;
    print_global_processes(&rows);
    Ok(())
}

pub fn execute_forward(name: String) -> Result<()> {
    let (config_path, config) = crate::engine::config::require_config()?;
    let (url, pid) = forward_start(&config_path, &config, &name)?;
    println!("🌐 Cloudflare Tunnel active for [{}]:", name);
    println!("   URL: {}", url);
    println!("   PID: {}", pid);
    Ok(())
}

pub fn execute_unforward(name: String) -> Result<()> {
    let (config_path, _) = crate::engine::config::require_config()?;
    if forward_stop(&config_path, &name)? {
        println!("🛑 Stopped Cloudflare tunnel for [{}]", name);
    } else {
        println!("   No active tunnel found for [{}]", name);
    }
    Ok(())
}
