pub mod agent;
pub mod database;
pub mod rules;

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use crate::engine::config::Config;
use crate::engine::logs::read_tail;
use crate::engine::state::{is_alive, read_state};
use crate::engine::supervisor::resolve_names;

pub use agent::{
    build_system_diagnostics_prompt, extract_fix_commands, find_available_agent,
    list_detected_agents, run_ai_prompt, DiagnosticContext, ProcessSnapshot,
};
pub use rules::RuleCategory;

const RECENT_LOG_LINES: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticEngine {
    RuleEngine(String),
    AiAgent(String),
    HeuristicFallback,
}

impl DiagnosticEngine {
    pub fn label(&self) -> String {
        match self {
            Self::RuleEngine(rule_name) => format!("Internal Rule Engine [{}]", rule_name),
            Self::AiAgent(agent) => format!("AI Agent CLI [{}]", agent),
            Self::HeuristicFallback => "Generic Log Inspector".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticReport {
    pub process_name: String,
    pub running: bool,
    pub pid: Option<i32>,
    pub port: Option<u16>,
    pub category: Option<RuleCategory>,
    pub root_cause: String,
    pub explanation: String,
    pub fix_command: Option<String>,
    pub engine_used: DiagnosticEngine,
    pub matched_line: Option<String>,
    pub log_file: PathBuf,
}

pub fn collect_snapshots(
    config_path: &Path,
    config: &Config,
    target_name: Option<&str>,
) -> Result<Vec<ProcessSnapshot>> {
    let state = read_state(config_path);
    let names = resolve_names(config, target_name)?;
    let mut snapshots = Vec::new();

    for name in names {
        let def = config
            .processes
            .get(&name)
            .ok_or_else(|| anyhow!("Process '{}' not found in config", name))?;

        let proc_entry = state.processes.get(&name);
        let pid = proc_entry.map(|p| p.pid);
        let alive = pid.map(is_alive).unwrap_or(false);

        let log_file = proc_entry
            .map(|p| PathBuf::from(&p.log_file))
            .unwrap_or_else(|| {
                crate::engine::paths::project_log_dir(config_path)
                    .map(|d| d.join(format!("{}.log", name)))
                    .unwrap_or_else(|_| PathBuf::from(format!("{}.log", name)))
            });

        let recent_logs = if log_file.is_file() {
            read_tail(&log_file, RECENT_LOG_LINES)
        } else {
            String::new()
        };

        snapshots.push(ProcessSnapshot {
            name,
            command: def.cmd.clone(),
            cwd: def.cwd.clone().unwrap_or_else(|| ".".to_string()),
            port: def.port,
            running: alive,
            pid,
            exit_code: None,
            recent_logs,
        });
    }

    Ok(snapshots)
}

pub fn run_ai_system_check(
    config_path: &Path,
    config: &Config,
    target_name: Option<&str>,
    agent_override: Option<&str>,
) -> Result<(String, Vec<String>, String)> {
    let snapshots = collect_snapshots(config_path, config, target_name)?;
    let config_raw = std::fs::read_to_string(config_path).ok();

    let agent = find_available_agent(agent_override)
        .ok_or_else(|| anyhow!("No AI agent CLI found on system (checked: agy, claude, codex, gemini, ollama)."))?;

    let prompt = build_system_diagnostics_prompt(&snapshots, config_raw.as_deref(), target_name);
    let ai_response = run_ai_prompt(&agent, &prompt)?;
    let fixes = extract_fix_commands(&ai_response);

    Ok((ai_response, fixes, agent.name))
}

pub fn diagnose_all(
    config_path: &Path,
    config: &Config,
    target_name: Option<&str>,
    force_ai: bool,
    agent_override: Option<&str>,
) -> Result<Vec<DiagnosticReport>> {
    let state = read_state(config_path);
    let names = resolve_names(config, target_name)?;
    let config_raw = std::fs::read_to_string(config_path).ok();

    let mut reports = Vec::new();

    for name in names {
        let def = config
            .processes
            .get(&name)
            .ok_or_else(|| anyhow!("Process '{}' not found in config", name))?;

        let proc_entry = state.processes.get(&name);
        let pid = proc_entry.map(|p| p.pid);
        let alive = pid.map(is_alive).unwrap_or(false);

        // If target_name is None and not force_ai, only diagnose services that are NOT running (crashed/down)
        if target_name.is_none() && alive && !force_ai {
            continue;
        }

        let log_file = proc_entry
            .map(|p| PathBuf::from(&p.log_file))
            .unwrap_or_else(|| {
                crate::engine::paths::project_log_dir(config_path)
                    .map(|d| d.join(format!("{}.log", name)))
                    .unwrap_or_else(|_| PathBuf::from(format!("{}.log", name)))
            });

        let recent_logs = if log_file.is_file() {
            read_tail(&log_file, RECENT_LOG_LINES)
        } else {
            "(No logs generated yet)".to_string()
        };

        let report = diagnose_service(
            &name,
            &def.cmd,
            def.cwd.as_deref().unwrap_or("."),
            def.port,
            alive,
            pid,
            &log_file,
            &recent_logs,
            force_ai,
            agent_override,
            config_raw.as_deref(),
        )?;

        reports.push(report);
    }

    Ok(reports)
}

pub fn diagnose_service(
    name: &str,
    cmd: &str,
    cwd: &str,
    port: Option<u16>,
    alive: bool,
    pid: Option<i32>,
    log_file: &Path,
    recent_logs: &str,
    force_ai: bool,
    agent_override: Option<&str>,
    config_snippet: Option<&str>,
) -> Result<DiagnosticReport> {
    if alive && !force_ai {
        return Ok(DiagnosticReport {
            process_name: name.to_string(),
            running: true,
            pid,
            port,
            category: None,
            root_cause: "Process is running normally.".to_string(),
            explanation: "No active crash detected. The process PID is alive and responding.".to_string(),
            fix_command: None,
            engine_used: DiagnosticEngine::HeuristicFallback,
            matched_line: None,
            log_file: log_file.to_path_buf(),
        });
    }

    // --- Tier 1: Local Rule & Database Matcher (Fast & Offline) ---
    if !force_ai {
        if let Some(rule_match) = database::evaluate_rules(recent_logs, None) {
            return Ok(DiagnosticReport {
                process_name: name.to_string(),
                running: alive,
                pid,
                port,
                category: Some(rule_match.category),
                root_cause: rule_match.rule_name.clone(),
                explanation: rule_match.explanation,
                fix_command: rule_match.fix_command,
                engine_used: DiagnosticEngine::RuleEngine(rule_match.rule_name),
                matched_line: Some(rule_match.matched_line),
                log_file: log_file.to_path_buf(),
            });
        }
    }

    // --- Tier 2: AI Agent Delegation (agy / claude / codex / gemini / ollama) ---
    let agent_desc = find_available_agent(agent_override);
    if let Some(agent) = agent_desc {
        let ctx = DiagnosticContext {
            process_name: name,
            command: cmd,
            cwd,
            port,
            exit_code: None,
            recent_logs,
            config_snippet,
        };

        match agent::run_ai_diagnostics(&agent, &ctx) {
            Ok(ai_response) => {
                let (root_cause, fix_cmd) = parse_ai_response(&ai_response);
                return Ok(DiagnosticReport {
                    process_name: name.to_string(),
                    running: alive,
                    pid,
                    port,
                    category: Some(RuleCategory::SyntaxOrPanic),
                    root_cause,
                    explanation: ai_response,
                    fix_command: fix_cmd,
                    engine_used: DiagnosticEngine::AiAgent(agent.name),
                    matched_line: None,
                    log_file: log_file.to_path_buf(),
                });
            }
            Err(e) if force_ai => {
                return Err(anyhow!("AI Agent '{}' execution failed: {}", agent.name, e));
            }
            Err(_) => {
                // If AI execution failed in non-forced mode, fallback to heuristic
            }
        }
    }

    // --- Tier 3: Fallback Heuristic when no rule matched and no AI available ---
    let last_lines: Vec<&str> = recent_logs
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    let fallback_line = last_lines
        .last()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Process status evaluated.".to_string());

    let root_cause = if alive {
        "Process Running (No issue detected)".to_string()
    } else {
        "Process Terminated / Crash".to_string()
    };

    Ok(DiagnosticReport {
        process_name: name.to_string(),
        running: alive,
        pid,
        port,
        category: Some(RuleCategory::SyntaxOrPanic),
        root_cause,
        explanation: format!(
            "Last recorded log output:\n{}",
            recent_logs
        ),
        fix_command: if alive { None } else { Some("procman doctor --ai".to_string()) },
        engine_used: DiagnosticEngine::HeuristicFallback,
        matched_line: Some(fallback_line),
        log_file: log_file.to_path_buf(),
    })
}

fn parse_ai_response(response: &str) -> (String, Option<String>) {
    let mut root_cause = "AI Root Cause Analysis".to_string();
    let mut fix_cmd = None;

    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("1.") || trimmed.to_lowercase().starts_with("root cause:") {
            root_cause = trimmed
                .trim_start_matches("1.")
                .trim_start_matches("Root cause:")
                .trim_start_matches("Root Cause:")
                .trim()
                .to_string();
        }
        if (trimmed.starts_with('`') && trimmed.ends_with('`'))
            || trimmed.starts_with("$ ")
            || trimmed.to_lowercase().starts_with("fix command:")
        {
            let clean = trimmed
                .trim_start_matches("$ ")
                .trim_start_matches("Fix command:")
                .trim_start_matches("Fix Command:")
                .trim_matches('`')
                .trim();
            if !clean.is_empty() {
                fix_cmd = Some(clean.to_string());
            }
        }
    }

    (root_cause, fix_cmd)
}

