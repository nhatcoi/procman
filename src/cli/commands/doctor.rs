use anyhow::Result;
use std::io::{self, Write};
use std::process::Command;

use crate::engine::config::require_config;
use crate::engine::doctor::{self, DiagnosticEngine, DiagnosticReport};

pub fn execute(
    name: Option<String>,
    spot: bool,
    force_ai: bool,
    agent_override: Option<String>,
    auto_fix: bool,
) -> Result<()> {
    let (config_path, config) = require_config()?;

    println!("🩺  PROCMAN DOCTOR & DIAGNOSTIC ASSISTANT");
    println!("══════════════════════════════════════════════════════════════════════");

    // List detected AI agents in the environment
    let detected_agents = doctor::list_detected_agents();
    if !detected_agents.is_empty() {
        let agent_names: Vec<&str> = detected_agents.iter().map(|a| a.name.as_str()).collect();
        println!("🤖 Detected AI Agent CLIs : {}", agent_names.join(", "));
    } else {
        println!("🤖 AI Agent CLIs          : None detected (Offline Rule Engine active)");
    }
    println!("──────────────────────────────────────────────────────────────────────");

    let use_ai = if force_ai {
        true
    } else if spot {
        false
    } else {
        prompt_mode_selection()?
    };

    if use_ai {
        println!();
        if let Some(target) = &name {
            println!("🤖 Initiating AI Check for service '{}'...", target);
        } else {
            println!("🤖 Initiating AI Check across all procman processes...");
        }

        match doctor::run_ai_system_check(
            &config_path,
            &config,
            name.as_deref(),
            agent_override.as_deref(),
        ) {
            Ok((ai_response, fixes, agent_name)) => {
                println!();
                println!("📋 AI DIAGNOSTIC REPORT [{}]", agent_name);
                println!("──────────────────────────────────────────────────────────────────────");
                println!("{}", ai_response.trim());
                println!("──────────────────────────────────────────────────────────────────────");

                execute_fixes(&fixes, auto_fix)?;
            }
            Err(e) => {
                eprintln!();
                eprintln!("⚠️  AI Check failed: {}", e);
                eprintln!("💡 You can run 'procman doctor -s' to perform offline Spot Check instead.");
            }
        }
    } else {
        let reports = doctor::diagnose_all(
            &config_path,
            &config,
            name.as_deref(),
            false,
            agent_override.as_deref(),
        )?;

        if reports.is_empty() {
            println!();
            println!("✨ All services in procman.yaml are running smoothly!");
            println!("   No crashed or down processes were detected.");
            println!();
            println!("💡 Tips:");
            println!("   • Select option [2] or run 'procman doctor --ai' for deep AI process analysis.");
            println!("   • Run 'procman doctor <name>' to inspect any specific service.");
            println!("══════════════════════════════════════════════════════════════════════");
            return Ok(());
        }

        let mut fixes_to_run: Vec<String> = Vec::new();

        for report in &reports {
            print_diagnostic_card(report);
            if let Some(fix) = &report.fix_command {
                fixes_to_run.push(fix.clone());
            }
        }

        execute_fixes(&fixes_to_run, auto_fix)?;
    }

    println!("══════════════════════════════════════════════════════════════════════");
    Ok(())
}

fn prompt_mode_selection() -> Result<bool> {
    println!("Select Diagnostic Mode:");
    println!("  [1] 🩺 Spot Check (Instant Rule Engine & Status Scanner) [Default]");
    println!("  [2] 🤖 AI Check   (AI Deep Process Analysis & Health Assessment)");
    println!();
    print!("Select mode [1/2] (press Enter for [1]): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();

    if trimmed == "2" || trimmed == "ai" || trimmed == "a" || trimmed == "2." {
        Ok(true)
    } else {
        Ok(false)
    }
}

fn execute_fixes(fixes: &[String], auto_fix: bool) -> Result<()> {
    if fixes.is_empty() {
        return Ok(());
    }

    println!();
    println!("🛠️  REMEDIATION ACTIONS");
    println!("──────────────────────────────────────────────────────────────────────");

    for fix in fixes {
        let should_run = if auto_fix {
            true
        } else {
            prompt_confirm(&format!("Execute fix command: '{}'?", fix))?
        };

        if should_run {
            println!("🚀 Running: {}", fix);
            let status = Command::new("sh")
                .arg("-c")
                .arg(fix)
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("✅ Fix command completed successfully!");
                }
                Ok(s) => {
                    eprintln!("⚠️ Fix command exited with status: {}", s);
                }
                Err(e) => {
                    eprintln!("❌ Failed to execute fix command: {}", e);
                }
            }
        }
    }
    Ok(())
}

fn print_diagnostic_card(report: &DiagnosticReport) {
    let status_badge = if report.running {
        if let Some(pid) = report.pid {
            format!("🟢 RUNNING (PID: {})", pid)
        } else {
            "🟢 RUNNING".to_string()
        }
    } else {
        "🔴 DOWN / CRASHED".to_string()
    };

    let icon = report
        .category
        .map(|c| c.icon())
        .unwrap_or("🩺");

    let cat_name = report
        .category
        .map(|c| c.display_name())
        .unwrap_or("General");

    println!();
    println!(
        "{} [{}]  Status: {}  |  Category: {}",
        icon, report.process_name, status_badge, cat_name
    );
    println!("   Diagnostic Engine : {}", report.engine_used.label());

    if let Some(port) = report.port {
        println!("   Configured Port   : {}", port);
    }
    println!("   Log File          : {:?}", report.log_file);

    println!("   Root Cause        : {}", report.root_cause);

    if let Some(line) = &report.matched_line {
        println!("   Matched Evidence  : {}", line);
    }

    if let DiagnosticEngine::AiAgent(_) = report.engine_used {
        println!("   AI Analysis Report:");
        for l in report.explanation.lines() {
            println!("     {}", l);
        }
    } else if report.explanation != report.root_cause {
        println!("   Explanation       : {}", report.explanation);
    }

    if let Some(fix) = &report.fix_command {
        println!("   💡 Suggested Fix   : \x1b[1;32m{}\x1b[0m", fix);
    }
    println!("──────────────────────────────────────────────────────────────────────");
}

fn prompt_confirm(prompt: &str) -> Result<bool> {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}
