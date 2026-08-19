use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

use crate::engine::paths::{global_data_root, global_state_root};
use crate::engine::supervisor::{force_kill_by_pid, scan_global_processes};

pub fn execute(yes: bool, purge: bool) -> Result<()> {
    println!("⚠️  Preparing to uninstall procman...");

    let all_procs = scan_global_processes()?;
    let running: Vec<_> = all_procs.into_iter().filter(|p| p.running).collect();

    if !running.is_empty() {
        println!(
            "\n⚠️  There are {} active process(es) currently managed by procman:",
            running.len()
        );
        for p in &running {
            println!(
                "   - [{}] {} (PID: {})",
                p.project_key,
                p.service_name,
                p.pid.unwrap_or(0)
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
            if let Some(pid) = p.pid {
                let _ = force_kill_by_pid(pid);
            }
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

    println!("\n🗑️  Removing procman binary via cargo uninstall...");
    let status = Command::new("cargo")
        .args(["uninstall", "procman"])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("   Successfully uninstalled procman via cargo.");
        }
        _ => {
            println!("   Note: `cargo uninstall procman` did not succeed or procman was installed manually.");
            if let Ok(current_exe) = env::current_exe() {
                if let Err(e) = fs::remove_file(&current_exe) {
                    println!(
                        "   Could not remove binary at {:?}: {}. Please remove manually.",
                        current_exe, e
                    );
                } else {
                    println!("   Removed binary at {:?}", current_exe);
                }
            }
        }
    }

    println!("\n✨ procman uninstalled successfully. Thank you for using procman!");
    Ok(())
}
