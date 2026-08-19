use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

use crate::paths::{global_data_root, global_state_root};
use crate::process_manager::{force_kill_by_pid, scan_global_processes};

pub fn run_uninstall(yes: bool, purge: bool) -> Result<()> {
    println!("⚠️  Preparing to uninstall procman...");

    let running = scan_global_processes().unwrap_or_default();
    if !running.is_empty() {
        println!(
            "\n⚠️  There are {} active process(es) currently managed by procman:",
            running.len()
        );
        for p in &running {
            println!(
                "   - [{}] {} (PID: {})",
                p.project_key, p.service_name, p.pid
            );
        }
        println!("   Uninstalling will stop all of these processes.");
    }

    if !yes {
        print!("\n❓ Are you sure you want to uninstall procman? [y/N]: ");
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read user confirmation")?;

        let ans = input.trim().to_lowercase();
        if ans != "y" && ans != "yes" {
            println!("Uninstall cancelled.");
            return Ok(());
        }
    }

    if !running.is_empty() {
        println!("\n🛑 Stopping running processes...");
        for p in &running {
            let _ = force_kill_by_pid(p.pid);
        }
        println!("   Stopped all active processes.");
    }

    if purge {
        println!("\n🧹 Purging state and logs directories...");
        let state_root = global_state_root();
        if state_root.exists() {
            let _ = fs::remove_dir_all(&state_root);
            println!("   Deleted state directory: {:?}", state_root);
        }
        let data_root = global_data_root();
        if data_root.exists() {
            let _ = fs::remove_dir_all(&data_root);
            println!("   Deleted logs directory: {:?}", data_root);
        }
    }

    println!("\n🗑️  Removing procman executable...");
    let mut removed = false;

    if let Ok(exe_path) = env::current_exe() {
        if exe_path.is_file() {
            if fs::remove_file(&exe_path).is_ok() {
                println!("   Removed executable at {:?}", exe_path);
                removed = true;
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let cargo_bin = home.join(".cargo").join("bin").join("procman");
        if cargo_bin.is_file() && fs::remove_file(&cargo_bin).is_ok() {
            println!("   Removed executable at {:?}", cargo_bin);
            removed = true;
        }
        let local_bin = home.join(".local").join("bin").join("procman");
        if local_bin.is_file() && fs::remove_file(&local_bin).is_ok() {
            println!("   Removed executable at {:?}", local_bin);
            removed = true;
        }
    }

    let _ = Command::new("cargo")
        .args(["uninstall", "procman"])
        .output();

    if removed {
        println!("\n✅ procman has been successfully uninstalled from your system.");
    } else {
        println!("\n✅ procman cleanup complete.");
        println!("   If the binary still exists on your PATH, you can remove it manually with: `rm $(which procman)`");
    }

    Ok(())
}
