use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;

use crate::engine::config::Config;
use crate::engine::doctor::{find_available_agent, run_ai_prompt};

pub fn generate_config_via_ai(
    root_dir: &Path,
    agent_override: Option<&str>,
) -> Result<(String, String)> {
    let agent = find_available_agent(agent_override).ok_or_else(|| {
        anyhow!("No AI agent CLI found on system (checked: agy, claude, codex, gemini, ollama).")
    })?;

    let overview = collect_project_overview(root_dir);
    let prompt = build_init_prompt(&overview);

    let raw_response = run_ai_prompt(&agent, &prompt)?;
    let yaml_content = extract_yaml_block(&raw_response)?;

    // Validate that the returned YAML is a valid procman Config
    let parsed: Config = serde_yaml::from_str(&yaml_content).with_context(|| {
        format!(
            "AI Agent '{}' generated invalid YAML structure:\n{}",
            agent.name, yaml_content
        )
    })?;

    if parsed.processes.is_empty() {
        return Err(anyhow!(
            "AI Agent '{}' generated a configuration with 0 processes.",
            agent.name
        ));
    }

    Ok((yaml_content, agent.name))
}

fn collect_project_overview(root_dir: &Path) -> String {
    let mut out = String::new();

    // 1. Directory Tree Summary
    out.push_str("### DIRECTORY STRUCTURE:\n");
    if let Ok(entries) = fs::read_dir(root_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if should_skip_for_ai(name) {
                continue;
            }
            if path.is_dir() {
                out.push_str(&format!("📁 {}/\n", name));
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub in sub_entries.flatten().take(10) {
                        let sname = sub.file_name();
                        let sname_str = sname.to_string_lossy();
                        if !should_skip_for_ai(&sname_str) {
                            out.push_str(&format!("   └── {}\n", sname_str));
                        }
                    }
                }
            } else {
                out.push_str(&format!("📄 {}\n", name));
            }
        }
    }
    out.push('\n');

    // 2. Manifest file contents
    let key_files = [
        "package.json",
        "Cargo.toml",
        "go.mod",
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yaml",
        "compose.yml",
        "Makefile",
        "pyproject.toml",
        "requirements.txt",
        "manage.py",
        ".env.example",
        "frontend/package.json",
        "backend/package.json",
        "apps/web/package.json",
        "apps/api/package.json",
    ];

    out.push_str("### KEY MANIFEST & CONFIG FILES:\n");
    for kf in key_files {
        let p = root_dir.join(kf);
        if p.is_file() {
            if let Ok(content) = fs::read_to_string(&p) {
                let truncated: String = content.lines().take(80).collect::<Vec<&str>>().join("\n");
                out.push_str(&format!("--- FILE: {} ---\n{}\n\n", kf, truncated));
            }
        }
    }

    out
}

fn build_init_prompt(overview: &str) -> String {
    format!(
        "You are an expert DevOps engineer and process manager specialist for 'procman'.\n\
        Analyze the project structure and manifests below, then generate a clean, optimal 'procman.yaml' configuration.\n\n\
        {}\n\
        --- PROCMAN.YAML SCHEMA SPECIFICATION ---\n\
        processes:\n\
          <process_name>:\n\
            cmd: <startup command, e.g. 'npm run dev', 'cargo run', 'uvicorn main:app --reload'>\n\
            cwd: <optional relative folder path if not root, e.g. 'frontend' or 'backend'>\n\
            port: <optional network port number integer, e.g. 3000, 8000, 5173>\n\
            depends_on: [<optional list of dependency service names to wait for>]\n\
            watch: true # enable auto-restart on code changes\n\n\
        --- INSTRUCTIONS ---\n\
        1. Identify all backend, frontend, worker, database/docker services.\n\
        2. Set proper `depends_on` (e.g. frontend depends_on api or db).\n\
        3. Set accurate `port` numbers based on package.json/framework conventions or .env.example.\n\
        4. Return ONLY the valid YAML content wrapped in ```yaml ... ``` code block. Do NOT include extraneous conversational explanations.",
        overview
    )
}

fn extract_yaml_block(raw: &str) -> Result<String> {
    if let Some(start) = raw.find("```yaml") {
        let after = &raw[start + 7..];
        if let Some(end) = after.find("```") {
            return Ok(after[..end].trim().to_string());
        }
    }
    if let Some(start) = raw.find("```yml") {
        let after = &raw[start + 6..];
        if let Some(end) = after.find("```") {
            return Ok(after[..end].trim().to_string());
        }
    }
    if let Some(start) = raw.find("```") {
        let after = &raw[start + 3..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if candidate.contains("processes:") {
                return Ok(candidate.to_string());
            }
        }
    }
    if raw.contains("processes:") {
        return Ok(raw.trim().to_string());
    }

    Err(anyhow!("No valid YAML block found in AI response"))
}

fn should_skip_for_ai(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | ".next"
            | ".nuxt"
            | "dist"
            | "build"
            | ".venv"
            | "venv"
            | "coverage"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_yaml_block() {
        let ai_resp = "Here is your config:\n```yaml\nprocesses:\n  api:\n    cmd: cargo run\n    port: 8000\n```\nEnjoy!";
        let yaml = extract_yaml_block(ai_resp).unwrap();
        assert!(yaml.contains("processes:"));
        assert!(yaml.contains("cmd: cargo run"));
    }
}
