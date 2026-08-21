use anyhow::{anyhow, Context, Result};
use std::process::{Command, Stdio};

const CANDIDATE_AGENTS: [&str; 5] = ["agy", "claude", "codex", "gemini", "ollama"];

#[derive(Debug, Clone)]
pub struct AiAgentDescriptor {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct DiagnosticContext<'a> {
    pub process_name: &'a str,
    pub command: &'a str,
    pub cwd: &'a str,
    pub port: Option<u16>,
    pub exit_code: Option<i32>,
    pub recent_logs: &'a str,
    pub config_snippet: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub port: Option<u16>,
    pub running: bool,
    pub pid: Option<i32>,
    pub exit_code: Option<i32>,
    pub recent_logs: String,
}

pub fn find_available_agent(override_name: Option<&str>) -> Option<AiAgentDescriptor> {
    if let Some(name) = override_name {
        if let Some(p) = find_binary_in_path(name) {
            return Some(AiAgentDescriptor {
                name: name.to_string(),
                path: p,
            });
        }
        return None;
    }

    for agent in CANDIDATE_AGENTS.iter() {
        if let Some(p) = find_binary_in_path(agent) {
            return Some(AiAgentDescriptor {
                name: agent.to_string(),
                path: p,
            });
        }
    }

    None
}

pub fn list_detected_agents() -> Vec<AiAgentDescriptor> {
    let mut detected = Vec::new();
    for agent in CANDIDATE_AGENTS.iter() {
        if let Some(p) = find_binary_in_path(agent) {
            detected.push(AiAgentDescriptor {
                name: agent.to_string(),
                path: p,
            });
        }
    }
    detected
}

pub fn run_ai_prompt(
    agent: &AiAgentDescriptor,
    prompt: &str,
) -> Result<String> {
    let mut cmd = Command::new(&agent.path);

    match agent.name.as_str() {
        "claude" => {
            cmd.arg("-p").arg(prompt);
        }
        "ollama" => {
            cmd.arg("run").arg("mistral").arg(prompt);
        }
        _ => {
            cmd.arg(prompt);
        }
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd
        .output()
        .with_context(|| format!("Failed to execute AI agent CLI '{}'", agent.name))?;

    let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() && stdout_str.is_empty() {
        return Err(anyhow!(
            "AI Agent '{}' exited with status {}: {}",
            agent.name,
            output.status,
            stderr_str
        ));
    }

    if stdout_str.is_empty() {
        Ok(stderr_str)
    } else {
        Ok(stdout_str)
    }
}

pub fn run_ai_diagnostics(
    agent: &AiAgentDescriptor,
    ctx: &DiagnosticContext,
) -> Result<String> {
    let prompt = build_diagnostics_prompt(ctx);
    run_ai_prompt(agent, &prompt)
}

pub fn build_system_diagnostics_prompt(
    snapshots: &[ProcessSnapshot],
    config_snippet: Option<&str>,
    target_process: Option<&str>,
) -> String {
    let mut procs_text = String::new();
    for s in snapshots {
        let status_str = if s.running {
            if let Some(pid) = s.pid {
                format!("RUNNING (PID: {})", pid)
            } else {
                "RUNNING".to_string()
            }
        } else if let Some(code) = s.exit_code {
            format!("DOWN (Exit Code: {})", code)
        } else {
            "DOWN / CRASHED".to_string()
        };

        let port_str = s.port.map(|p| p.to_string()).unwrap_or_else(|| "None".to_string());

        let logs_summary = if s.recent_logs.trim().is_empty() {
            "(No logs generated yet)".to_string()
        } else {
            s.recent_logs.trim().to_string()
        };

        procs_text.push_str(&format!(
            "### Service: {}\n- Command: `{}`\n- Working Dir: `{}`\n- Port: {}\n- Status: {}\n- Logs (recent tail):\n```\n{}\n```\n\n",
            s.name, s.command, s.cwd, port_str, status_str, logs_summary
        ));
    }

    let target_focus = if let Some(t) = target_process {
        format!("\nFOCUS INSTRUCTION: Deeply diagnose the '{}' service while checking integration with other services.\n", t)
    } else {
        String::new()
    };

    let cfg_text = config_snippet.unwrap_or("N/A");

    format!(
        "You are an expert development diagnostics assistant for 'procman'.\n\
        Analyze the process snapshots and logs to provide a CONCISE, HIGHLY VISUAL health report.\n{}\n\
        --- PROCMAN.YAML CONFIG ---\n{}\n\n\
        --- PROCESS SNAPSHOTS & LOGS ---\n{}\n\
        --- OUTPUT FORMAT REQUIREMENTS ---\n\
        Output your diagnosis directly and cleanly with markdown formatting:\n\
        1. 📊 **Process Status & Health Summary** (Markdown Table with columns: Service | Status [🟢 UP / 🟡 WARN / 🔴 DOWN] | Port | Quick Health Assessment [1 line]).\n\
        2. 🔍 **Key Findings / Warnings / Crashes** (1-3 short bullet points highlighting root causes of crashes, port collisions, runtime warnings, or missing dependencies. If completely healthy, write '✨ All processes operating smoothly.').\n\
        3. 💡 **Actionable Fix Commands** (If an issue is found, provide the exact executable shell command on a line starting with '$ ' e.g. `$ npm install express` or `$ procman kill-port 3000`).",
        target_focus, cfg_text, procs_text
    )
}

pub fn build_diagnostics_prompt(ctx: &DiagnosticContext) -> String {
    let exit_str = ctx
        .exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "Unknown / Crashed".to_string());

    let port_str = ctx
        .port
        .map(|p| p.to_string())
        .unwrap_or_else(|| "None".to_string());

    let config_str = ctx.config_snippet.unwrap_or("N/A");

    format!(
        "You are an expert development diagnostics assistant for procman.\n\
        Analyze the failure, explain the root cause concisely, and give the exact fix command.\n\n\
        --- PROCESS METADATA ---\n\
        Service Name : {}\n\
        Command      : {}\n\
        Working Dir  : {}\n\
        Port         : {}\n\
        Exit Code    : {}\n\n\
        --- PROCMAN.YAML CONFIG ---\n\
        {}\n\n\
        --- RECENT LOGS (LAST 50 LINES) ---\n\
        {}\n\n\
        --- INSTRUCTIONS ---\n\
        1. State Root Cause (RCA) in 1-2 clear sentences.\n\
        2. Provide the exact Fix Command (e.g. bash/shell command) that resolves this issue on a line starting with '$ '.",
        ctx.process_name, ctx.command, ctx.cwd, port_str, exit_str, config_str, ctx.recent_logs
    )
}

pub fn extract_fix_commands(ai_response: &str) -> Vec<String> {
    let mut fixes = Vec::new();
    for line in ai_response.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("$ ") {
            let cmd = trimmed.trim_start_matches("$ ").trim();
            if !cmd.is_empty() && !fixes.contains(&cmd.to_string()) {
                fixes.push(cmd.to_string());
            }
        } else if trimmed.to_lowercase().starts_with("fix command:") {
            let cmd = trimmed
                .split_at(12)
                .1
                .trim()
                .trim_matches('`')
                .trim_start_matches("$ ")
                .trim();
            if !cmd.is_empty() && !fixes.contains(&cmd.to_string()) {
                fixes.push(cmd.to_string());
            }
        }
    }
    fixes
}

fn find_binary_in_path(binary_name: &str) -> Option<String> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(binary_name);
            if candidate.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = candidate.metadata() {
                        if meta.permissions().mode() & 0o111 != 0 {
                            return Some(candidate.to_string_lossy().to_string());
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_binary() {
        // 'sh' or 'cargo' should exist on any standard development machine
        let res = find_binary_in_path("sh");
        assert!(res.is_some());
    }

    #[test]
    fn test_build_prompt() {
        let ctx = DiagnosticContext {
            process_name: "api",
            command: "npm run dev",
            cwd: "/home/user/app",
            port: Some(3000),
            exit_code: Some(1),
            recent_logs: "Error: Cannot find module 'express'",
            config_snippet: Some("processes:\n  api:\n    cmd: npm run dev"),
        };
        let p = build_diagnostics_prompt(&ctx);
        assert!(p.contains("Service Name : api"));
        assert!(p.contains("Error: Cannot find module 'express'"));
    }

    #[test]
    fn test_build_system_diagnostics_prompt() {
        let snapshots = vec![
            ProcessSnapshot {
                name: "api".to_string(),
                command: "cargo run".to_string(),
                cwd: ".".to_string(),
                port: Some(8000),
                running: true,
                pid: Some(12345),
                exit_code: None,
                recent_logs: "Server started on port 8000".to_string(),
            },
            ProcessSnapshot {
                name: "worker".to_string(),
                command: "python worker.py".to_string(),
                cwd: ".".to_string(),
                port: None,
                running: false,
                pid: None,
                exit_code: Some(1),
                recent_logs: "ModuleNotFoundError: No module named 'redis'".to_string(),
            },
        ];
        let p = build_system_diagnostics_prompt(&snapshots, Some("processes:\n  api: ..."), None);
        assert!(p.contains("### Service: api"));
        assert!(p.contains("### Service: worker"));
        assert!(p.contains("RUNNING (PID: 12345)"));
        assert!(p.contains("DOWN (Exit Code: 1)"));
    }

    #[test]
    fn test_extract_fix_commands() {
        let ai_text = r#"
1. 📊 **Process Status & Health Summary**
| Service | Status | Port | Quick Health Note |
| api | 🟢 UP | 8000 | Healthy |
| worker | 🔴 DOWN | None | Missing redis module |

2. 🔍 **Key Findings**
- Worker crashed because redis is missing.

3. 💡 **Actionable Fix Commands**
$ pip install redis
Fix command: `procman restart worker`
"#;
        let fixes = extract_fix_commands(ai_text);
        assert_eq!(fixes.len(), 2);
        assert_eq!(fixes[0], "pip install redis");
        assert_eq!(fixes[1], "procman restart worker");
    }
}
