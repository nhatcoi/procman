use std::collections::HashMap;
use std::fs;
use std::path::Path;
use regex::Regex;
use serde_json::Value;

use crate::engine::config::{ProcessDef, WatchConfig};

pub struct ScannedProject {
    pub processes: HashMap<String, ProcessDef>,
    pub detected_frameworks: Vec<String>,
}

pub fn scan_directory(root_dir: &Path) -> ScannedProject {
    let mut processes = HashMap::new();
    let mut detected_frameworks = Vec::new();

    // 1. Scan root directory
    scan_single_folder(root_dir, None, &mut processes, &mut detected_frameworks);

    // 2. Scan immediate subdirectories (monorepo / multi-service patterns)
    if let Ok(entries) = fs::read_dir(root_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let folder_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if should_skip_dir(&folder_name) {
                    continue;
                }

                // Check nested subfolders if this is apps/ or packages/
                if folder_name == "apps" || folder_name == "packages" || folder_name == "services" {
                    if let Ok(nested_entries) = fs::read_dir(&path) {
                        for nested in nested_entries.flatten() {
                            let npath = nested.path();
                            if npath.is_dir() {
                                let nname = npath
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("");
                                if !should_skip_dir(nname) {
                                    let rel_path = format!("{}/{}", folder_name, nname);
                                    scan_single_folder(&npath, Some(&rel_path), &mut processes, &mut detected_frameworks);
                                }
                            }
                        }
                    }
                } else {
                    scan_single_folder(&path, Some(&folder_name), &mut processes, &mut detected_frameworks);
                }
            }
        }
    }

    // 3. Scan docker-compose.yml / compose.yaml
    scan_docker_compose(root_dir, &mut processes, &mut detected_frameworks);

    // 4. If no processes found, provide a fallback template
    if processes.is_empty() {
        processes.insert(
            "app".to_string(),
            ProcessDef {
                cmd: "echo 'Configure your startup command here' && sleep 3600".to_string(),
                cwd: None,
                env: HashMap::new(),
                port: None,
                forward: false,
                tunnel: false,
                free_port: false,
                log_file: None,
                depends_on: Vec::new(),
                watch: Some(WatchConfig::Enabled(true)),
                watch_ignore: Vec::new(),
            },
        );
        detected_frameworks.push("Generic Template".to_string());
    }

    ScannedProject {
        processes,
        detected_frameworks,
    }
}

fn scan_single_folder(
    dir: &Path,
    rel_cwd: Option<&str>,
    processes: &mut HashMap<String, ProcessDef>,
    frameworks: &mut Vec<String>,
) {
    let service_name_fallback = rel_cwd
        .map(|s| s.replace('/', "-"))
        .unwrap_or_else(|| "web".to_string());

    // --- A. Node.js / JavaScript / TypeScript (package.json) ---
    let pkg_json_path = dir.join("package.json");
    if pkg_json_path.is_file() {
        if let Ok(content) = fs::read_to_string(&pkg_json_path) {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                let pkg_name = val.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.trim_start_matches('@').replace('/', "-"))
                    .unwrap_or_else(|| service_name_fallback.clone());

                let mut port = detect_port_from_env(dir);
                let mut dev_cmd = None;
                let package_manager = detect_package_manager(dir);

                // Detect scripts
                if let Some(scripts) = val.get("scripts").and_then(|s| s.as_object()) {
                    if scripts.contains_key("dev") {
                        dev_cmd = Some(format!("{} run dev", package_manager));
                    } else if scripts.contains_key("start") {
                        dev_cmd = Some(format!("{} start", package_manager));
                    } else if scripts.contains_key("serve") {
                        dev_cmd = Some(format!("{} run serve", package_manager));
                    }
                }

                // Detect frameworks
                let mut fw_detected = "Node.js".to_string();
                let deps_str = serde_json::to_string(&val).unwrap_or_default();

                if deps_str.contains("\"next\"") {
                    fw_detected = "Next.js".to_string();
                    if port.is_none() { port = Some(3000); }
                } else if deps_str.contains("\"vite\"") {
                    fw_detected = "Vite".to_string();
                    if port.is_none() { port = Some(5173); }
                } else if deps_str.contains("\"@nestjs/core\"") {
                    fw_detected = "NestJS".to_string();
                    if port.is_none() { port = Some(3000); }
                    if dev_cmd.is_none() { dev_cmd = Some(format!("{} run start:dev", package_manager)); }
                } else if deps_str.contains("\"astro\"") {
                    fw_detected = "Astro".to_string();
                    if port.is_none() { port = Some(4321); }
                } else if deps_str.contains("\"@remix-run\"") {
                    fw_detected = "Remix".to_string();
                    if port.is_none() { port = Some(3000); }
                } else if deps_str.contains("\"nuxt\"") {
                    fw_detected = "Nuxt".to_string();
                    if port.is_none() { port = Some(3000); }
                } else if deps_str.contains("\"express\"") || deps_str.contains("\"fastify\"") || deps_str.contains("\"koa\"") {
                    fw_detected = "Node Backend".to_string();
                    if port.is_none() { port = Some(8000); }
                }

                frameworks.push(format!("{} ({})", fw_detected, rel_cwd.unwrap_or("root")));

                let final_cmd = dev_cmd.unwrap_or_else(|| format!("{} start", package_manager));
                let svc_name = sanitize_name(&pkg_name);

                processes.insert(
                    svc_name,
                    ProcessDef {
                        cmd: final_cmd,
                        cwd: rel_cwd.map(|s| s.to_string()),
                        env: HashMap::new(),
                        port,
                        forward: false,
                        tunnel: false,
                        free_port: true,
                        log_file: None,
                        depends_on: Vec::new(),
                        watch: Some(WatchConfig::Enabled(true)),
                        watch_ignore: vec![
                            ".git".to_string(),
                            "node_modules".to_string(),
                            ".next".to_string(),
                            "dist".to_string(),
                        ],
                    },
                );
                return;
            }
        }
    }

    // --- B. Rust (Cargo.toml) ---
    let cargo_path = dir.join("Cargo.toml");
    if cargo_path.is_file() {
        let mut svc_name = rel_cwd
            .and_then(|p| p.split('/').last())
            .unwrap_or("app")
            .to_string();

        if let Ok(content) = fs::read_to_string(&cargo_path) {
            let re_name = Regex::new(r#"(?m)^\s*name\s*=\s*"([^"]+)""#).unwrap();
            if let Some(cap) = re_name.captures(&content) {
                if let Some(m) = cap.get(1) {
                    svc_name = m.as_str().to_string();
                }
            }
        }

        let port = detect_port_from_env(dir);
        frameworks.push(format!("Rust Cargo ({})", rel_cwd.unwrap_or("root")));

        processes.insert(
            sanitize_name(&svc_name),
            ProcessDef {
                cmd: "cargo run".to_string(),
                cwd: rel_cwd.map(|s| s.to_string()),
                env: HashMap::new(),
                port,
                forward: false,
                tunnel: false,
                free_port: true,
                log_file: None,
                depends_on: Vec::new(),
                watch: Some(WatchConfig::Enabled(true)),
                watch_ignore: vec![".git".to_string(), "target".to_string()],
            },
        );
        return;
    }

    // --- C. Go (go.mod) ---
    let go_mod_path = dir.join("go.mod");
    if go_mod_path.is_file() {
        let mut svc_name = rel_cwd
            .and_then(|p| p.split('/').last())
            .unwrap_or("api")
            .to_string();

        if let Ok(content) = fs::read_to_string(&go_mod_path) {
            let re_mod = Regex::new(r"(?m)^\s*module\s+([^\s\n\r]+)").unwrap();
            if let Some(cap) = re_mod.captures(&content) {
                if let Some(m) = cap.get(1) {
                    if let Some(last_part) = m.as_str().split('/').last() {
                        svc_name = last_part.to_string();
                    }
                }
            }
        }

        let port = detect_port_from_env(dir).or(Some(8080));
        frameworks.push(format!("Go ({})", rel_cwd.unwrap_or("root")));

        processes.insert(
            sanitize_name(&svc_name),
            ProcessDef {
                cmd: "go run .".to_string(),
                cwd: rel_cwd.map(|s| s.to_string()),
                env: HashMap::new(),
                port,
                forward: false,
                tunnel: false,
                free_port: true,
                log_file: None,
                depends_on: Vec::new(),
                watch: Some(WatchConfig::Enabled(true)),
                watch_ignore: vec![".git".to_string()],
            },
        );
        return;
    }

    // --- D. Python (pyproject.toml / requirements.txt / manage.py / main.py) ---
    let is_python = dir.join("pyproject.toml").is_file()
        || dir.join("requirements.txt").is_file()
        || dir.join("Pipfile").is_file()
        || dir.join("manage.py").is_file()
        || dir.join("main.py").is_file();

    if is_python {
        let svc_name = rel_cwd
            .and_then(|p| p.split('/').last())
            .unwrap_or("api");
        let mut port = detect_port_from_env(dir);
        let mut cmd = "python main.py".to_string();
        let mut fw_name = "Python";

        if dir.join("manage.py").is_file() {
            fw_name = "Django";
            port = port.or(Some(8000));
            cmd = "python manage.py runserver 8000".to_string();
        } else if let Ok(req) = fs::read_to_string(dir.join("requirements.txt")) {
            if req.contains("fastapi") || req.contains("uvicorn") {
                fw_name = "FastAPI";
                port = port.or(Some(8000));
                cmd = "uvicorn main:app --reload --port 8000".to_string();
            } else if req.contains("flask") {
                fw_name = "Flask";
                port = port.or(Some(5000));
                cmd = "flask run -p 5000".to_string();
            } else if req.contains("streamlit") {
                fw_name = "Streamlit";
                port = port.or(Some(8501));
                cmd = "streamlit run app.py".to_string();
            }
        }

        frameworks.push(format!("{} ({})", fw_name, rel_cwd.unwrap_or("root")));

        processes.insert(
            sanitize_name(svc_name),
            ProcessDef {
                cmd,
                cwd: rel_cwd.map(|s| s.to_string()),
                env: HashMap::new(),
                port,
                forward: false,
                tunnel: false,
                free_port: true,
                log_file: None,
                depends_on: Vec::new(),
                watch: Some(WatchConfig::Enabled(true)),
                watch_ignore: vec![
                    ".git".to_string(),
                    "__pycache__".to_string(),
                    ".venv".to_string(),
                    "venv".to_string(),
                ],
            },
        );
    }
}

fn scan_docker_compose(
    root_dir: &Path,
    processes: &mut HashMap<String, ProcessDef>,
    frameworks: &mut Vec<String>,
) {
    let compose_files = ["docker-compose.yml", "docker-compose.yaml", "compose.yaml", "compose.yml"];
    for fname in compose_files {
        let cpath = root_dir.join(fname);
        if cpath.is_file() {
            if let Ok(content) = fs::read_to_string(&cpath) {
                let re_service = Regex::new(r"(?m)^  ([a-zA-Z0-9_-]+):").unwrap();
                for cap in re_service.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        let svc = m.as_str();
                        // Only add db/infrastructure services (redis, postgres, mysql, mongo)
                        if matches!(svc, "redis" | "postgres" | "db" | "mysql" | "mongo" | "rabbitmq" | "mailhog" | "localstack") {
                            let port = match svc {
                                "postgres" | "db" => Some(5432),
                                "redis" => Some(6379),
                                "mysql" => Some(3306),
                                "mongo" => Some(27017),
                                "rabbitmq" => Some(5672),
                                _ => None,
                            };

                            let svc_name = format!("docker-{}", svc);
                            if !processes.contains_key(&svc_name) {
                                frameworks.push(format!("Docker Compose [{}]", svc));
                                processes.insert(
                                    svc_name,
                                    ProcessDef {
                                        cmd: format!("docker compose up {}", svc),
                                        cwd: None,
                                        env: HashMap::new(),
                                        port,
                                        forward: false,
                                        tunnel: false,
                                        free_port: false,
                                        log_file: None,
                                        depends_on: Vec::new(),
                                        watch: None,
                                        watch_ignore: Vec::new(),
                                    },
                                );
                            }
                        }
                    }
                }
            }
            break;
        }
    }
}

fn detect_package_manager(dir: &Path) -> &'static str {
    if dir.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if dir.join("yarn.lock").is_file() {
        "yarn"
    } else if dir.join("bun.lockb").is_file() || dir.join("bun.lock").is_file() {
        "bun"
    } else {
        "npm"
    }
}

fn detect_port_from_env(dir: &Path) -> Option<u16> {
    let env_files = [".env", ".env.local", ".env.example", ".env.development"];
    let re_port = Regex::new(r"(?i)(?:PORT|API_PORT|SERVER_PORT|APP_PORT)\s*=\s*(\d+)").ok()?;

    for ef in env_files {
        let p = dir.join(ef);
        if p.is_file() {
            if let Ok(content) = fs::read_to_string(&p) {
                if let Some(cap) = re_port.captures(&content) {
                    if let Some(m) = cap.get(1) {
                        if let Ok(port_num) = m.as_str().parse::<u16>() {
                            return Some(port_num);
                        }
                    }
                }
            }
        }
    }
    None
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".next"
            | ".nuxt"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".venv"
            | "venv"
            | ".idea"
            | ".vscode"
            | "coverage"
            | "tmp"
            | "temp"
    )
}

fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();

    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "service".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("@scope/my-app"), "scope-my-app");
        assert_eq!(sanitize_name("frontend_api!"), "frontend_api");
        assert_eq!(sanitize_name(""), "service");
    }

    #[test]
    fn test_scan_package_json() {
        let temp = std::env::temp_dir().join("procman_test_scanner_node");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let mut f = File::create(temp.join("package.json")).unwrap();
        writeln!(
            f,
            r#"{{"name":"my-vite-app","scripts":{{"dev":"vite"}},"dependencies":{{"vite":"^5.0.0"}}}}"#
        ).unwrap();

        let scanned = scan_directory(&temp);
        assert!(scanned.processes.contains_key("my-vite-app"));
        let p = scanned.processes.get("my-vite-app").unwrap();
        assert_eq!(p.cmd, "npm run dev");
        assert_eq!(p.port, Some(5173));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_scan_monorepo_multi_service() {
        let temp = std::env::temp_dir().join("procman_test_scanner_monorepo");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join("apps/web")).unwrap();
        fs::create_dir_all(temp.join("apps/api")).unwrap();

        // Web Next.js app
        let mut f_web = File::create(temp.join("apps/web/package.json")).unwrap();
        writeln!(
            f_web,
            r#"{{"name":"web","scripts":{{"dev":"next dev"}},"dependencies":{{"next":"14.0.0"}}}}"#
        ).unwrap();

        // API Cargo app
        let mut f_api = File::create(temp.join("apps/api/Cargo.toml")).unwrap();
        writeln!(f_api, r#"[package]
name = "api"
version = "0.1.0""#).unwrap();

        // docker-compose.yml with postgres
        let mut f_dc = File::create(temp.join("docker-compose.yml")).unwrap();
        writeln!(f_dc, "services:\n  postgres:\n    image: postgres:15\n    ports:\n      - '5432:5432'").unwrap();

        let scanned = scan_directory(&temp);
        assert!(scanned.processes.contains_key("web"));
        assert_eq!(scanned.processes.get("web").unwrap().cwd, Some("apps/web".to_string()));
        assert_eq!(scanned.processes.get("web").unwrap().port, Some(3000));

        assert!(scanned.processes.contains_key("api"));
        assert_eq!(scanned.processes.get("api").unwrap().cwd, Some("apps/api".to_string()));

        assert!(scanned.processes.contains_key("docker-postgres"));
        assert_eq!(scanned.processes.get("docker-postgres").unwrap().port, Some(5432));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_scan_python_fastapi() {
        let temp = std::env::temp_dir().join("procman_test_scanner_python");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let mut f = File::create(temp.join("requirements.txt")).unwrap();
        writeln!(f, "fastapi\nuvicorn\npydantic").unwrap();

        let scanned = scan_directory(&temp);
        assert!(scanned.processes.contains_key("api"));
        let p = scanned.processes.get("api").unwrap();
        assert_eq!(p.cmd, "uvicorn main:app --reload --port 8000");
        assert_eq!(p.port, Some(8000));

        let _ = fs::remove_dir_all(&temp);
    }
}
