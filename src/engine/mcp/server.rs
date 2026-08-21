use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use super::protocol::{
    JsonRpcRequest, JsonRpcResponse, McpResource, McpTool, McpToolCallResult,
};
use crate::engine::config::{find_config_path, load_config, Config};
use crate::engine::doctor;
use crate::engine::init;
use crate::engine::logs::read_tail;
use crate::engine::metrics::ProcessMetrics;
use crate::engine::paths::project_log_dir;
use crate::engine::supervisor;

pub fn run_stdio_server(project_dir: Option<&Path>) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = stdin.lock();

    let mut line_buf = String::new();
    while reader.read_line(&mut line_buf)? > 0 {
        let trimmed = line_buf.trim();
        if !trimmed.is_empty() {
            if let Some(resp_json) = handle_json_rpc(trimmed, project_dir) {
                writeln!(stdout, "{}", resp_json)?;
                stdout.flush()?;
            }
        }
        line_buf.clear();
    }

    Ok(())
}

pub fn handle_json_rpc(raw_line: &str, project_dir: Option<&Path>) -> Option<String> {
    let req: JsonRpcRequest = match serde_json::from_str(raw_line) {
        Ok(r) => r,
        Err(e) => {
            let resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
            return serde_json::to_string(&resp).ok();
        }
    };

    let id = req.id.clone();
    let resp = match req.method.as_str() {
        "initialize" => {
            let result = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "procman",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });
            JsonRpcResponse::success(id, result)
        }
        "notifications/initialized" | "initialized" => {
            // Notifications do not require a response if id is null
            if id.is_some() {
                JsonRpcResponse::success(id, json!({}))
            } else {
                return None;
            }
        }
        "ping" => JsonRpcResponse::success(id, json!({})),
        "tools/list" => {
            let tools = get_tool_definitions();
            JsonRpcResponse::success(id, json!({ "tools": tools }))
        }
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let tool_res = dispatch_tool_call(name, &arguments, project_dir);
            JsonRpcResponse::success(id, serde_json::to_value(tool_res).unwrap_or(json!({})))
        }
        "resources/list" => {
            let resources = get_resource_definitions(project_dir);
            JsonRpcResponse::success(id, json!({ "resources": resources }))
        }
        "resources/read" => {
            let params = req.params.unwrap_or(json!({}));
            let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            let read_res = dispatch_resource_read(uri, project_dir);
            match read_res {
                Ok(contents) => JsonRpcResponse::success(id, json!({ "contents": contents })),
                Err(e) => JsonRpcResponse::error(id, -32602, format!("Resource error: {}", e)),
            }
        }
        _ => JsonRpcResponse::error(id, -32601, format!("Method not found: {}", req.method)),
    };

    serde_json::to_string(&resp).ok()
}

fn get_tool_definitions() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "procman_status".to_string(),
            description: "Retrieve status of all configured services in current project, including PID, port, uptime, cpu%, memory(MB), and liveness probe.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Optional specific process name to query"
                    }
                }
            }),
        },
        McpTool {
            name: "procman_start".to_string(),
            description: "Start all or a specific process in the background. Waits for dependencies if configured in depends_on.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Process name (starts all if omitted)"
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Force kill port conflicts before starting"
                    }
                }
            }),
        },
        McpTool {
            name: "procman_stop".to_string(),
            description: "Gracefully stop all or a specific running process and its tunnels.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Process name (stops all if omitted)"
                    }
                }
            }),
        },
        McpTool {
            name: "procman_restart".to_string(),
            description: "Restart all or a specific process.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Process name (restarts all if omitted)"
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Force port clearing"
                    }
                }
            }),
        },
        McpTool {
            name: "procman_logs".to_string(),
            description: "Read recent stdout/stderr logs of a process.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Process name"
                    },
                    "lines": {
                        "type": "integer",
                        "description": "Number of tail lines to retrieve (default: 60)"
                    }
                }
            }),
        },
        McpTool {
            name: "procman_doctor".to_string(),
            description: "Run multi-tier diagnostics on processes to identify crash root causes, port collisions, missing packages, or runtime warnings.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Optional process name"
                    },
                    "ai": {
                        "type": "boolean",
                        "description": "Run deep AI check across processes"
                    }
                }
            }),
        },
        McpTool {
            name: "procman_kill_port".to_string(),
            description: "Force kill any process occupying a specified TCP port.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["port"],
                "properties": {
                    "port": {
                        "type": "integer",
                        "description": "TCP port number"
                    }
                }
            }),
        },
        McpTool {
            name: "procman_ps".to_string(),
            description: "List all active procman background processes across all projects on the entire system.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpTool {
            name: "procman_init".to_string(),
            description: "Scan project directory manifests and auto-generate an optimal procman.yaml configuration.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "dir": {
                        "type": "string",
                        "description": "Target directory path (default: current project)"
                    },
                    "ai": {
                        "type": "boolean",
                        "description": "Use AI generator"
                    }
                }
            }),
        },
    ]
}

fn dispatch_tool_call(name: &str, args: &Value, project_dir: Option<&Path>) -> McpToolCallResult {
    match name {
        "procman_status" => {
            let target_name = args.get("name").and_then(|v| v.as_str());
            match get_project_config(project_dir) {
                Ok((cfg_path, cfg)) => {
                    let mut metrics = ProcessMetrics::new();
                    match supervisor::status_with_metrics(&cfg_path, &cfg, target_name, &mut metrics) {
                        Ok(rows) => {
                            let data: Vec<Value> = rows.into_iter().map(|r| {
                                json!({
                                    "name": r.name,
                                    "running": r.running,
                                    "pid": r.pid,
                                    "port": r.port,
                                    "cpu_percent": r.cpu,
                                    "memory_mb": r.memory_mb,
                                    "uptime": r.uptime,
                                    "tunnel_url": r.tunnel_url,
                                    "log_file": r.log_file,
                                })
                            }).collect();
                            McpToolCallResult::text(serde_json::to_string_pretty(&data).unwrap_or_default())
                        }
                        Err(e) => McpToolCallResult::error(format!("Failed to retrieve status: {}", e)),
                    }
                }
                Err(e) => McpToolCallResult::error(e.to_string()),
            }
        }
        "procman_start" => {
            let target_name = args.get("name").and_then(|v| v.as_str());
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            match get_project_config(project_dir) {
                Ok((cfg_path, cfg)) => {
                    match supervisor::start(&cfg_path, &cfg, target_name, force) {
                        Ok(()) => McpToolCallResult::text(format!(
                            "Successfully started {}",
                            target_name.unwrap_or("all configured processes")
                        )),
                        Err(e) => McpToolCallResult::error(format!("Failed to start: {}", e)),
                    }
                }
                Err(e) => McpToolCallResult::error(e.to_string()),
            }
        }
        "procman_stop" => {
            let target_name = args.get("name").and_then(|v| v.as_str());
            match get_project_config(project_dir) {
                Ok((cfg_path, cfg)) => {
                    match supervisor::stop(&cfg_path, &cfg, target_name) {
                        Ok(()) => McpToolCallResult::text(format!(
                            "Successfully stopped {}",
                            target_name.unwrap_or("all configured processes")
                        )),
                        Err(e) => McpToolCallResult::error(format!("Failed to stop: {}", e)),
                    }
                }
                Err(e) => McpToolCallResult::error(e.to_string()),
            }
        }
        "procman_restart" => {
            let target_name = args.get("name").and_then(|v| v.as_str());
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            match get_project_config(project_dir) {
                Ok((cfg_path, cfg)) => {
                    match supervisor::restart(&cfg_path, &cfg, target_name, force) {
                        Ok(()) => McpToolCallResult::text(format!(
                            "Successfully restarted {}",
                            target_name.unwrap_or("all configured processes")
                        )),
                        Err(e) => McpToolCallResult::error(format!("Failed to restart: {}", e)),
                    }
                }
                Err(e) => McpToolCallResult::error(e.to_string()),
            }
        }
        "procman_logs" => {
            let proc_name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return McpToolCallResult::error("Missing required parameter 'name'"),
            };
            let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(60) as usize;

            match get_project_config(project_dir) {
                Ok((cfg_path, _)) => {
                    let log_file = project_log_dir(&cfg_path)
                        .map(|d| d.join(format!("{}.log", proc_name)))
                        .unwrap_or_else(|_| PathBuf::from(format!("{}.log", proc_name)));

                    if log_file.is_file() {
                        let text = read_tail(&log_file, lines);
                        McpToolCallResult::text(text)
                    } else {
                        McpToolCallResult::text(format!("(No log file found at {:?})", log_file))
                    }
                }
                Err(e) => McpToolCallResult::error(e.to_string()),
            }
        }
        "procman_doctor" => {
            let target_name = args.get("name").and_then(|v| v.as_str());
            let use_ai = args.get("ai").and_then(|v| v.as_bool()).unwrap_or(false);

            match get_project_config(project_dir) {
                Ok((cfg_path, cfg)) => {
                    if use_ai {
                        match doctor::run_ai_system_check(&cfg_path, &cfg, target_name, None) {
                            Ok((report, fixes, agent)) => {
                                let mut out = format!("### AI Doctor Report [{}]\n\n{}", agent, report);
                                if !fixes.is_empty() {
                                    out.push_str("\n\n### Suggested Fix Commands:\n");
                                    for f in fixes {
                                        out.push_str(&format!("- `{}`\n", f));
                                    }
                                }
                                McpToolCallResult::text(out)
                            }
                            Err(e) => McpToolCallResult::error(format!("AI Doctor failed: {}", e)),
                        }
                    } else {
                        match doctor::diagnose_all(&cfg_path, &cfg, target_name, false, None) {
                            Ok(reports) => {
                                if reports.is_empty() {
                                    McpToolCallResult::text("✨ All services are running smoothly! No crashed processes detected.")
                                } else {
                                    let data: Vec<Value> = reports.into_iter().map(|r| {
                                        json!({
                                            "process": r.process_name,
                                            "running": r.running,
                                            "root_cause": r.root_cause,
                                            "explanation": r.explanation,
                                            "fix_command": r.fix_command,
                                            "engine": r.engine_used.label(),
                                        })
                                    }).collect();
                                    McpToolCallResult::text(serde_json::to_string_pretty(&data).unwrap_or_default())
                                }
                            }
                            Err(e) => McpToolCallResult::error(format!("Diagnostic failed: {}", e)),
                        }
                    }
                }
                Err(e) => McpToolCallResult::error(e.to_string()),
            }
        }
        "procman_kill_port" => {
            let port = match args.get("port").and_then(|v| v.as_u64()) {
                Some(p) => p as u16,
                None => return McpToolCallResult::error("Missing required parameter 'port'"),
            };
            supervisor::kill_port(port);
            McpToolCallResult::text(format!("Signal sent to kill any process on port {}", port))
        }
        "procman_ps" => {
            let mut metrics = ProcessMetrics::new();
            match supervisor::scan_global_processes_with_metrics(&mut metrics) {
                Ok(procs) => {
                    let data: Vec<Value> = procs.into_iter().map(|p| {
                        json!({
                            "project_key": p.project_key,
                            "service_name": p.service_name,
                            "running": p.running,
                            "pid": p.pid,
                            "port": p.port,
                            "cpu_percent": p.cpu,
                            "memory_mb": p.memory_mb,
                            "uptime": p.uptime,
                            "config_path": p.config_path,
                        })
                    }).collect();
                    McpToolCallResult::text(serde_json::to_string_pretty(&data).unwrap_or_default())
                }
                Err(e) => McpToolCallResult::error(format!("Failed to query global processes: {}", e)),
            }
        }
        "procman_init" => {
            let dir_str = args.get("dir").and_then(|v| v.as_str()).unwrap_or(".");
            let force_ai = args.get("ai").and_then(|v| v.as_bool()).unwrap_or(false);
            let target_path = PathBuf::from(dir_str);

            match init::generate_initial_config(&target_path, force_ai, None) {
                Ok((yaml, engine)) => {
                    let msg = format!("Generated procman.yaml via {}:\n\n```yaml\n{}\n```", engine, yaml);
                    McpToolCallResult::text(msg)
                }
                Err(e) => McpToolCallResult::error(format!("Failed to generate config: {}", e)),
            }
        }
        _ => McpToolCallResult::error(format!("Unknown tool: {}", name)),
    }
}

fn get_resource_definitions(project_dir: Option<&Path>) -> Vec<McpResource> {
    let mut list = vec![McpResource {
        uri: "procman://processes".to_string(),
        name: "Process Status Table".to_string(),
        description: Some("Current status of all procman processes".to_string()),
        mime_type: Some("application/json".to_string()),
    }];

    if let Ok((_cfg_path, cfg)) = get_project_config(project_dir) {
        for name in cfg.processes.keys() {
            list.push(McpResource {
                uri: format!("procman://logs/{}", name),
                name: format!("Logs for {}", name),
                description: Some(format!("Live tail logs for process '{}'", name)),
                mime_type: Some("text/plain".to_string()),
            });
        }
    }

    list
}

fn dispatch_resource_read(uri: &str, project_dir: Option<&Path>) -> Result<Vec<Value>> {
    if uri == "procman://processes" {
        let (cfg_path, cfg) = get_project_config(project_dir)?;
        let mut metrics = ProcessMetrics::new();
        let rows = supervisor::status_with_metrics(&cfg_path, &cfg, None, &mut metrics)?;
        let json_text = serde_json::to_string_pretty(&rows)?;
        return Ok(vec![json!({
            "uri": uri,
            "mimeType": "application/json",
            "text": json_text
        })]);
    }

    if let Some(proc_name) = uri.strip_prefix("procman://logs/") {
        let (cfg_path, _) = get_project_config(project_dir)?;
        let log_file = project_log_dir(&cfg_path)
            .map(|d| d.join(format!("{}.log", proc_name)))
            .unwrap_or_else(|_| PathBuf::from(format!("{}.log", proc_name)));

        let content = if log_file.is_file() {
            fs::read_to_string(&log_file).unwrap_or_else(|_| "(Failed to read log)".to_string())
        } else {
            "(No logs generated yet)".to_string()
        };

        return Ok(vec![json!({
            "uri": uri,
            "mimeType": "text/plain",
            "text": content
        })]);
    }

    Err(anyhow!("Unknown resource URI: {}", uri))
}

fn get_project_config(project_dir: Option<&Path>) -> Result<(PathBuf, Config)> {
    let cfg_path = find_config_path(project_dir)
        .ok_or_else(|| anyhow!("No procman.yaml found in target directory. Run 'procman init' first."))?;
    let cfg = load_config(&cfg_path)?;
    Ok((cfg_path, cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_initialize() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("\"name\":\"procman\""));
        assert!(resp.contains("\"protocolVersion\""));
    }

    #[test]
    fn test_handle_tools_list() {
        let raw = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("procman_status"));
        assert!(resp.contains("procman_start"));
        assert!(resp.contains("procman_logs"));
        assert!(resp.contains("procman_doctor"));
    }

    #[test]
    fn test_handle_ping() {
        let raw = r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("\"result\":{}"));
    }

    #[test]
    fn test_handle_tool_call_unknown() {
        let raw = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"unknown_tool","arguments":{}}}"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("Unknown tool"));
    }

    #[test]
    fn test_handle_tool_call_kill_port() {
        let raw = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"procman_kill_port","arguments":{"port":9999}}}"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("Signal sent to kill any process on port 9999"));
    }

    #[test]
    fn test_handle_resources_list() {
        let raw = r#"{"jsonrpc":"2.0","id":6,"method":"resources/list","params":{}}"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("procman://processes"));
    }

    #[test]
    fn test_handle_invalid_json() {
        let raw = r#"not valid json"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("-32700"));
        assert!(resp.contains("Parse error"));
    }

    #[test]
    fn test_handle_method_not_found() {
        let raw = r#"{"jsonrpc":"2.0","id":7,"method":"unsupported_method"}"#;
        let resp = handle_json_rpc(raw, None).unwrap();
        assert!(resp.contains("-32601"));
        assert!(resp.contains("Method not found"));
    }
}
