use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::engine::doctor;
use crate::engine::init;

pub fn execute(
    dir: String,
    force_ai: bool,
    agent_override: Option<String>,
    yes: bool,
    force: bool,
) -> Result<()> {
    let target_dir = PathBuf::from(&dir)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&dir));

    if !target_dir.is_dir() {
        return Err(anyhow!("Target path '{:?}' is not a directory", target_dir));
    }

    println!("🪄  PROCMAN PROJECT INITIALIZER");
    println!("══════════════════════════════════════════════════════════════════════");
    println!("📁 Target Directory : {:?}", target_dir);

    // List detected AI agents in the environment
    let detected_agents = doctor::list_detected_agents();
    if !detected_agents.is_empty() {
        let agent_names: Vec<&str> = detected_agents.iter().map(|a| a.name.as_str()).collect();
        println!("🤖 AI Agent CLIs    : {}", agent_names.join(", "));
    } else {
        println!("🤖 AI Agent CLIs    : None detected (Offline Heuristic Scanner active)");
    }
    println!("──────────────────────────────────────────────────────────────────────");

    let out_file = target_dir.join("procman.yaml");
    if out_file.is_file() && !force {
        print!("⚠️  'procman.yaml' already exists in this directory. Overwrite? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("🛑 Initialization cancelled.");
            println!("══════════════════════════════════════════════════════════════════════");
            return Ok(());
        }
    }

    println!("🔍 Scanning project structure and analyzing manifests...");
    let (yaml_content, engine_label) = init::generate_initial_config(
        &target_dir,
        force_ai,
        agent_override.as_deref(),
    )?;

    println!();
    println!("✨ Generated procman.yaml [{}]", engine_label);
    println!("──────────────────────────────────────────────────────────────────────");
    println!("{}", yaml_content.trim());
    println!("──────────────────────────────────────────────────────────────────────");

    if !yes {
        print!("Write configuration to '{:?}'? [Y/n]: ", out_file.file_name().unwrap_or_default());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();
        if trimmed == "n" || trimmed == "no" {
            println!("🛑 Initialization cancelled.");
            println!("══════════════════════════════════════════════════════════════════════");
            return Ok(());
        }
    }

    fs::write(&out_file, &yaml_content)
        .with_context(|| format!("Failed to write configuration to {:?}", out_file))?;

    println!();
    println!("✅ Successfully created 'procman.yaml'!");
    println!();
    println!("🚀 Next Steps:");
    println!("   • Run 'procman' or 'procman status' to inspect configured processes.");
    println!("   • Run 'procman start' to start all background services.");
    println!("   • Run 'procman ui' to open the interactive TUI dashboard.");
    println!("══════════════════════════════════════════════════════════════════════");

    Ok(())
}
