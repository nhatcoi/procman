use anyhow::{anyhow, Context, Result};
use chrono::Utc;
#[cfg(unix)]
use nix::sys::signal::{kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::net::{SocketAddr, TcpStream};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::config::Config;
use super::metrics::ProcessMetrics;
use super::paths::project_log_dir;
use super::state::{is_alive, read_state, write_state, ProcEntry, State};
use crate::tunnels::cloudflare::forward_start;

const SIGTERM_TIMEOUT: Duration = Duration::from_millis(1000);
#[cfg(unix)]
const SIGKILL_TIMEOUT: Duration = Duration::from_millis(800);
const POLL_INTERVAL: Duration = Duration::from_millis(30);
const DEPENDENCY_PORT_TIMEOUT: Duration = Duration::from_secs(30);
const DEPENDENCY_PROBE_INTERVAL: Duration = Duration::from_millis(150);

#[cfg(unix)]
const SHELL_BIN: &str = "sh";
#[cfg(unix)]
const SHELL_FLAG: &str = "-c";

#[cfg(windows)]
const SHELL_BIN: &str = "cmd.exe";
#[cfg(windows)]
const SHELL_FLAG: &str = "/C";

const LOG_FILE_EXTENSION: &str = "log";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StatusRow {
    pub name: String,
    pub running: bool,
    pub pid: Option<i32>,
    pub cpu: Option<f32>,
    pub memory_mb: Option<u64>,
    pub uptime: Option<String>,
    pub port: Option<u16>,
    pub log_file: String,
    pub tunnel_url: Option<String>,
    pub tunnel_pid: Option<i32>,
}

pub fn resolve_names(config: &Config, name: Option<&str>) -> Result<Vec<String>> {
    if let Some(n) = name {
        if !config.processes.contains_key(n) {
            return Err(anyhow!("Unknown process \"{}\" (check procman.yaml)", n));
        }
        Ok(vec![n.to_string()])
    } else {
        let mut names: Vec<String> = config.processes.keys().cloned().collect();
        names.sort();
        Ok(names)
    }
}

pub fn resolve_dependency_order(config: &Config, target_name: Option<&str>) -> Result<Vec<String>> {
    // 1. Validate all dependency names exist
    for (name, def) in &config.processes {
        for dep in &def.depends_on {
            if !config.processes.contains_key(dep) {
                return Err(anyhow!(
                    "Unknown dependency \"{}\" referenced by process \"{}\" (check procman.yaml)",
                    dep,
                    name
                ));
            }
        }
    }

    // 2. Identify the set of needed nodes
    let mut needed: HashSet<String> = HashSet::new();
    if let Some(target) = target_name {
        if !config.processes.contains_key(target) {
            return Err(anyhow!(
                "Unknown process \"{}\" (check procman.yaml)",
                target
            ));
        }
        let mut queue = VecDeque::new();
        queue.push_back(target.to_string());
        needed.insert(target.to_string());

        while let Some(current) = queue.pop_front() {
            if let Some(def) = config.processes.get(&current) {
                for dep in &def.depends_on {
                    if needed.insert(dep.clone()) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
    } else {
        needed = config.processes.keys().cloned().collect();
    }

    // 3. Cycle detection & Topological Sort using DFS
    let mut visited_state: HashMap<String, u8> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    fn dfs(
        node: &str,
        config: &Config,
        needed: &HashSet<String>,
        visited_state: &mut HashMap<String, u8>,
        order: &mut Vec<String>,
        path: &mut Vec<String>,
    ) -> Result<()> {
        visited_state.insert(node.to_string(), 1);
        path.push(node.to_string());

        if let Some(def) = config.processes.get(node) {
            let mut deps = def.depends_on.clone();
            deps.sort();

            for dep in &deps {
                if needed.contains(dep) {
                    match visited_state.get(dep).copied().unwrap_or(0) {
                        1 => {
                            path.push(dep.clone());
                            let cycle_start = path.iter().position(|p| p == dep).unwrap_or(0);
                            let cycle_str = path[cycle_start..].join(" -> ");
                            return Err(anyhow!("Cyclic dependency detected: {}", cycle_str));
                        }
                        0 => {
                            dfs(dep, config, needed, visited_state, order, path)?;
                        }
                        _ => {}
                    }
                }
            }
        }

        path.pop();
        visited_state.insert(node.to_string(), 2);
        order.push(node.to_string());
        Ok(())
    }

    let mut sorted_needed: Vec<String> = needed.iter().cloned().collect();
    sorted_needed.sort();

    let mut cycle_path: Vec<String> = Vec::new();
    for node in &sorted_needed {
        if visited_state.get(node).copied().unwrap_or(0) == 0 {
            dfs(
                node,
                config,
                &needed,
                &mut visited_state,
                &mut order,
                &mut cycle_path,
            )?;
        }
    }

    Ok(order)
}

pub fn wait_for_port_ready(port: u16, timeout: Duration) -> bool {
    let addr_str = format!("127.0.0.1:{}", port);
    let Ok(socket_addr) = addr_str.parse::<SocketAddr>() else {
        return false;
    };
    let deadline = Instant::now() + timeout;
    let probe_timeout = Duration::from_millis(150);

    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&socket_addr, probe_timeout).is_ok() {
            return true;
        }
        thread::sleep(DEPENDENCY_PROBE_INTERVAL);
    }
    false
}

pub fn is_service_ready(config: &Config, name: &str, pid: i32) -> bool {
    if !is_alive(pid) {
        return false;
    }
    if let Some(def) = config.processes.get(name) {
        if let Some(port) = def.port {
            let addr_str = format!("127.0.0.1:{}", port);
            if let Ok(socket_addr) = addr_str.parse::<SocketAddr>() {
                return TcpStream::connect_timeout(&socket_addr, Duration::from_millis(60)).is_ok();
            }
        }
    }
    true
}

pub fn log_file_for(config_path: &Path, name: &str) -> Result<PathBuf> {
    Ok(project_log_dir(config_path)?.join(format!("{}.{}", name, LOG_FILE_EXTENSION)))
}

#[cfg(unix)]
fn kill_group(pid: i32, signal: Signal, timeout: Duration) -> Result<()> {
    if pid <= 0 {
        return Ok(());
    }
    let _ = kill(Pid::from_raw(-pid), signal);

    let deadline = Instant::now() + timeout;
    while is_alive(pid) && Instant::now() < deadline {
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

#[cfg(windows)]
fn kill_group(pid: i32, timeout: Duration) -> Result<()> {
    if pid <= 0 {
        return Ok(());
    }
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let deadline = Instant::now() + timeout;
    while is_alive(pid) && Instant::now() < deadline {
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

#[cfg(unix)]
pub fn kill_port(port: u16) {
    if port == 0 {
        return;
    }
    let _ = Command::new("fuser")
        .args(["-k", "-9", &format!("{}/tcp", port)])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = Command::new(SHELL_BIN)
        .arg("-c")
        .arg(format!(
            "lsof -ti :{} 2>/dev/null | xargs -r kill -9 2>/dev/null",
            port
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(windows)]
pub fn kill_port(port: u16) {
    if port == 0 {
        return;
    }
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Get-NetTCPConnection -LocalPort {} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess | ForEach-Object {{ Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }}", port),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(unix)]
pub fn stop_by_pid(pid: i32) -> Result<()> {
    if !is_alive(pid) {
        return Ok(());
    }
    kill_group(pid, Signal::SIGTERM, SIGTERM_TIMEOUT)?;
    if is_alive(pid) {
        kill_group(pid, Signal::SIGKILL, SIGKILL_TIMEOUT)?;
    }
    Ok(())
}

#[cfg(windows)]
pub fn stop_by_pid(pid: i32) -> Result<()> {
    if !is_alive(pid) {
        return Ok(());
    }
    kill_group(pid, SIGTERM_TIMEOUT)?;
    Ok(())
}

#[cfg(unix)]
pub fn force_kill_by_pid(pid: i32) -> Result<()> {
    if pid <= 0 {
        return Ok(());
    }
    let _ = kill(Pid::from_raw(-pid), Signal::SIGKILL);
    let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
    Ok(())
}

#[cfg(windows)]
pub fn force_kill_by_pid(pid: i32) -> Result<()> {
    if pid <= 0 {
        return Ok(());
    }
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

fn resolve_cwd(config_path: &Path, rel_or_abs: Option<&str>) -> PathBuf {
    let base = config_path.parent().unwrap_or(config_path);
    match rel_or_abs {
        Some(p) => {
            let path = Path::new(p);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                base.join(path)
            }
        }
        None => base.to_path_buf(),
    }
}

fn start_one(config_path: &Path, config: &Config, name: &str, state: &mut State) -> Result<i32> {
    let def = config
        .processes
        .get(name)
        .ok_or_else(|| anyhow!("Process \"{}\" not found in config", name))?;

    if let Some(existing) = state.processes.get(name) {
        if is_alive(existing.pid) {
            println!(
                "   [{}] already running (PID {}) — use `procman restart` to restart",
                name, existing.pid
            );
            return Ok(existing.pid);
        }
    }

    if def.free_port {
        if let Some(port) = def.port {
            kill_port(port);
        }
    }

    let cwd = resolve_cwd(config_path, def.cwd.as_deref());
    let log_file = log_file_for(config_path, name)?;

    let header = format!(
        "\n----- procman start: {} @ {} -----\n$ {}\n",
        name,
        Utc::now().to_rfc3339(),
        def.cmd
    );
    let mut log_fd = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .with_context(|| format!("Failed to open log file {:?}", log_file))?;

    use std::io::Write;
    let _ = log_fd.write_all(header.as_bytes());

    let log_fd_err = log_fd
        .try_clone()
        .with_context(|| "Failed to clone log file descriptor")?;

    let mut cmd = Command::new(SHELL_BIN);
    cmd.arg(SHELL_FLAG)
        .arg(&def.cmd)
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_fd))
        .stderr(Stdio::from(log_fd_err));

    for (k, v) in &def.env {
        cmd.env(k, v);
    }

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn command for \"{}\"", name))?;
    let pid = child.id() as i32;

    thread::sleep(POLL_INTERVAL);
    if !is_alive(pid) {
        return Err(anyhow!(
            "Process \"{}\" exited immediately after spawn — check logs at {:?}",
            name,
            log_file
        ));
    }

    state.processes.insert(
        name.to_string(),
        ProcEntry {
            pid,
            cmd: def.cmd.clone(),
            cwd: cwd.to_string_lossy().to_string(),
            port: def.port,
            started_at: Utc::now().to_rfc3339(),
            log_file: log_file.to_string_lossy().to_string(),
        },
    );

    if def.forward || def.tunnel {
        let _ = forward_start(config_path, config, name);
    }

    Ok(pid)
}

pub fn start(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
    auto_forward: bool,
) -> Result<()> {
    let _ = super::registry::ProjectRegistry::register(config_path);
    let execution_order = resolve_dependency_order(config, name)?;
    let mut state = read_state(config_path);

    for n in &execution_order {
        // If service is already alive, skip restarting it unless requested
        if let Some(existing) = state.processes.get(n) {
            if is_alive(existing.pid) {
                if let Some(port) = existing.port {
                    if !is_service_ready(config, n, existing.pid) {
                        println!(
                            "   ⏳ Waiting for already running [{}] on port {}...",
                            n, port
                        );
                        let _ = wait_for_port_ready(port, DEPENDENCY_PORT_TIMEOUT);
                    }
                }
                println!("   [{}] already running (PID {})", n, existing.pid);
                continue;
            }
        }

        // Wait for all upstream dependencies of this service to be ready
        if let Some(def) = config.processes.get(n) {
            for dep in &def.depends_on {
                if let Some(dep_proc) = state.processes.get(dep) {
                    if is_alive(dep_proc.pid) {
                        if let Some(port) = dep_proc.port {
                            if !is_service_ready(config, dep, dep_proc.pid) {
                                println!(
                                    "   ⏳ Waiting for dependency [{}] to be ready on port {}...",
                                    dep, port
                                );
                                if wait_for_port_ready(port, DEPENDENCY_PORT_TIMEOUT) {
                                    println!(
                                        "   ✅ Dependency [{}] is ready on port {}",
                                        dep, port
                                    );
                                } else {
                                    eprintln!(
                                        "   ⚠️  Dependency [{}] did not open port {} within 30s (proceeding anyway)",
                                        dep, port
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        match start_one(config_path, config, n, &mut state) {
            Ok(pid) => {
                println!("🚀 Started [{}] (PID {})", n, pid);

                // If subsequent services in execution_order depend on this service, wait for its readiness
                let is_depended_on = execution_order.iter().any(|other| {
                    config
                        .processes
                        .get(other)
                        .map(|d| d.depends_on.contains(n))
                        .unwrap_or(false)
                });

                if is_depended_on {
                    if let Some(def) = config.processes.get(n) {
                        if let Some(port) = def.port {
                            println!("   ⏳ Waiting for [{}] to be ready on port {}...", n, port);
                            if wait_for_port_ready(port, DEPENDENCY_PORT_TIMEOUT) {
                                println!("   ✅ [{}] is ready on port {}", n, port);
                            } else {
                                eprintln!("   ⚠️  [{}] port {} not open after 30s", n, port);
                            }
                        } else {
                            thread::sleep(Duration::from_millis(300));
                        }
                    }
                }

                if auto_forward {
                    match forward_start(config_path, config, n) {
                        Ok((url, _)) => {
                            println!("   🌐 Cloudflare Tunnel: {}", url);
                        }
                        Err(e) => {
                            eprintln!("   ⚠️  Tunnel failed to start: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to start [{}]: {}", n, e);
            }
        }
    }

    write_state(config_path, &state)?;
    Ok(())
}

fn stop_one(name: &str, state: &mut State, force: bool) -> Result<()> {
    if let Some(entry) = state.processes.remove(name) {
        if is_alive(entry.pid) {
            if force {
                force_kill_by_pid(entry.pid)?;
                println!("🛑 Force-killed [{}] (PID {})", name, entry.pid);
            } else {
                stop_by_pid(entry.pid)?;
                println!("🛑 Stopped [{}] (PID {})", name, entry.pid);
            }
        } else {
            println!("   [{}] was not running (cleaned up stale state)", name);
        }
        if force {
            if let Some(port) = entry.port {
                kill_port(port);
            }
        }
    } else {
        println!("   [{}] is not tracked as running", name);
    }

    if let Some(tunnel) = state.tunnels.remove(name) {
        if is_alive(tunnel.pid) {
            let _ = stop_by_pid(tunnel.pid);
        }
    }

    Ok(())
}

pub fn stop(config_path: &Path, config: &Config, name: Option<&str>) -> Result<()> {
    let _ = super::registry::ProjectRegistry::register(config_path);
    let names = resolve_names(config, name)?;
    let mut state = read_state(config_path);

    for n in &names {
        stop_one(n, &mut state, false)?;
    }

    write_state(config_path, &state)?;
    Ok(())
}

pub fn force_stop(config_path: &Path, config: &Config, name: Option<&str>) -> Result<()> {
    let _ = super::registry::ProjectRegistry::register(config_path);
    let names = resolve_names(config, name)?;
    let mut state = read_state(config_path);

    for n in &names {
        stop_one(n, &mut state, true)?;
    }

    write_state(config_path, &state)?;
    Ok(())
}

pub fn restart(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
    auto_forward: bool,
) -> Result<()> {
    stop(config_path, config, name)?;
    thread::sleep(Duration::from_millis(200));
    start(config_path, config, name, auto_forward)?;
    Ok(())
}

pub fn uptime_string(started_at_rfc3339: &str) -> String {
    let Ok(started) = chrono::DateTime::parse_from_rfc3339(started_at_rfc3339) else {
        return "-".to_string();
    };
    let now = Utc::now();
    let duration = now.signed_duration_since(started);
    let secs = duration.num_seconds().max(0);

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn status(config_path: &Path, config: &Config, name: Option<&str>) -> Result<Vec<StatusRow>> {
    let mut metrics = ProcessMetrics::new_with_sample();
    status_with_metrics(config_path, config, name, &mut metrics)
}

pub fn status_with_metrics(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
    metrics: &mut ProcessMetrics,
) -> Result<Vec<StatusRow>> {
    let _ = super::registry::ProjectRegistry::register(config_path);
    let state = read_state(config_path);
    let names = resolve_names(config, name)?;

    let mut rows = Vec::new();
    for n in names {
        let def = config.processes.get(&n).unwrap();
        let proc_entry = state.processes.get(&n);
        let alive = proc_entry.map(|p| is_alive(p.pid)).unwrap_or(false);

        let (cpu, memory_mb) = if alive {
            metrics.query_tree(proc_entry.unwrap().pid)
        } else {
            (None, None)
        };

        let tunnel = state.tunnels.get(&n);
        let tunnel_alive = tunnel.map(|t| is_alive(t.pid)).unwrap_or(false);

        let default_log = log_file_for(config_path, &n)?.to_string_lossy().to_string();

        rows.push(StatusRow {
            name: n.clone(),
            running: alive,
            pid: if alive {
                proc_entry.map(|p| p.pid)
            } else {
                None
            },
            cpu,
            memory_mb,
            uptime: if alive {
                proc_entry.map(|p| uptime_string(&p.started_at))
            } else {
                None
            },
            port: if alive {
                proc_entry.and_then(|p| p.port)
            } else {
                def.port
            },
            tunnel_url: if tunnel_alive {
                tunnel.and_then(|t| t.url.clone())
            } else {
                None
            },
            tunnel_pid: if tunnel_alive {
                tunnel.map(|t| t.pid)
            } else {
                None
            },
            log_file: default_log,
        });
    }

    Ok(rows)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalProcRow {
    pub project_key: String,
    pub service_name: String,
    pub running: bool,
    pub pid: Option<i32>,
    pub cpu: Option<f32>,
    pub memory_mb: Option<u64>,
    pub uptime: Option<String>,
    pub port: Option<u16>,
    pub tunnel_url: Option<String>,
    pub cwd: String,
    pub config_path: Option<PathBuf>,
}

pub fn scan_global_processes() -> Result<Vec<GlobalProcRow>> {
    let mut metrics = ProcessMetrics::new_with_sample();
    scan_global_processes_with_metrics(&mut metrics)
}

pub fn scan_global_processes_with_metrics(
    metrics: &mut ProcessMetrics,
) -> Result<Vec<GlobalProcRow>> {
    let mut rows = Vec::new();
    let mut processed_keys = std::collections::HashSet::new();

    // 1. Scan from Known Projects Registry
    let registry = super::registry::ProjectRegistry::load();
    for proj in registry.get_all() {
        if !proj.config_path.is_file() {
            continue;
        }
        processed_keys.insert(proj.project_key.clone());

        if let Ok(cfg) = super::config::load_config(&proj.config_path) {
            let state = read_state(&proj.config_path);
            let mut service_names: Vec<String> = cfg.processes.keys().cloned().collect();
            service_names.sort();

            for service_name in &service_names {
                let def = cfg.processes.get(service_name).unwrap();
                let proc_entry = state.processes.get(service_name);
                let alive = proc_entry.map(|p| is_alive(p.pid)).unwrap_or(false);

                if alive {
                    let p = proc_entry.unwrap();
                    let (cpu, memory_mb) = metrics.query_tree(p.pid);
                    let tunnel = state.tunnels.get(service_name);
                    let tunnel_url = if tunnel.map(|t| is_alive(t.pid)).unwrap_or(false) {
                        tunnel.and_then(|t| t.url.clone())
                    } else {
                        None
                    };

                    rows.push(GlobalProcRow {
                        project_key: proj.project_key.clone(),
                        service_name: service_name.clone(),
                        running: true,
                        pid: Some(p.pid),
                        cpu,
                        memory_mb,
                        uptime: Some(uptime_string(&p.started_at)),
                        port: p.port.or(def.port),
                        tunnel_url,
                        cwd: p.cwd.clone(),
                        config_path: Some(proj.config_path.clone()),
                    });
                } else {
                    let default_cwd = def
                        .cwd
                        .as_ref()
                        .map(|c| proj.project_dir.join(c).to_string_lossy().to_string())
                        .unwrap_or_else(|| proj.project_dir.to_string_lossy().to_string());

                    rows.push(GlobalProcRow {
                        project_key: proj.project_key.clone(),
                        service_name: service_name.clone(),
                        running: false,
                        pid: None,
                        cpu: None,
                        memory_mb: None,
                        uptime: None,
                        port: def.port,
                        tunnel_url: None,
                        cwd: default_cwd,
                        config_path: Some(proj.config_path.clone()),
                    });
                }
            }
        }
    }

    // 2. Scan and auto-discover projects from ~/.local/state/procman/ that might not be in registry
    let state_root = super::paths::global_state_root();
    if state_root.is_dir() {
        if let Ok(entries) = fs::read_dir(&state_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let dir_key = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    if processed_keys.contains(&dir_key) {
                        continue;
                    }

                    let state_file = path.join("state.json");
                    if state_file.is_file() {
                        if let Ok(content) = fs::read_to_string(&state_file) {
                            if let Ok(state) = serde_json::from_str::<State>(&content) {
                                // Try to locate config_path from state.config_path or proc.cwd
                                let discovered_config: Option<PathBuf> = state
                                    .config_path
                                    .as_ref()
                                    .and_then(|p: &PathBuf| if p.is_file() { Some(p.clone()) } else { None })
                                    .or_else(|| {
                                        for proc in state.processes.values() {
                                            if let Some(cp) = super::config::find_config_path(Some(
                                                Path::new(&proc.cwd),
                                            )) {
                                                return Some(cp);
                                            }
                                        }
                                        None
                                    });

                                if let Some(ref config_path) = discovered_config {
                                    if let Ok(cfg) = super::config::load_config(config_path) {
                                        let _ = super::registry::ProjectRegistry::register(config_path);
                                        let actual_key = super::paths::project_key(config_path)
                                            .unwrap_or_else(|_| dir_key.clone());
                                        processed_keys.insert(actual_key.clone());
                                        processed_keys.insert(dir_key.clone());

                                        let project_dir = config_path
                                            .parent()
                                            .map(|p| p.to_path_buf())
                                            .unwrap_or_else(|| config_path.clone());
                                        let mut service_names: Vec<String> =
                                            cfg.processes.keys().cloned().collect();
                                        service_names.sort();

                                        for service_name in &service_names {
                                            let def = cfg.processes.get(service_name).unwrap();
                                            let proc_entry = state.processes.get(service_name);
                                            let alive = proc_entry
                                                .map(|p| is_alive(p.pid))
                                                .unwrap_or(false);

                                            if alive {
                                                let p = proc_entry.unwrap();
                                                let (cpu, memory_mb) = metrics.query_tree(p.pid);
                                                let tunnel = state.tunnels.get(service_name);
                                                let tunnel_url = if tunnel
                                                    .map(|t| is_alive(t.pid))
                                                    .unwrap_or(false)
                                                {
                                                    tunnel.and_then(|t| t.url.clone())
                                                } else {
                                                    None
                                                };

                                                rows.push(GlobalProcRow {
                                                    project_key: actual_key.clone(),
                                                    service_name: service_name.clone(),
                                                    running: true,
                                                    pid: Some(p.pid),
                                                    cpu,
                                                    memory_mb,
                                                    uptime: Some(uptime_string(&p.started_at)),
                                                    port: p.port.or(def.port),
                                                    tunnel_url,
                                                    cwd: p.cwd.clone(),
                                                    config_path: Some(config_path.clone()),
                                                });
                                            } else {
                                                let default_cwd = def
                                                    .cwd
                                                    .as_ref()
                                                    .map(|c| {
                                                        project_dir
                                                            .join(c)
                                                            .to_string_lossy()
                                                            .to_string()
                                                    })
                                                    .unwrap_or_else(|| {
                                                        project_dir.to_string_lossy().to_string()
                                                    });

                                                rows.push(GlobalProcRow {
                                                    project_key: actual_key.clone(),
                                                    service_name: service_name.clone(),
                                                    running: false,
                                                    pid: None,
                                                    cpu: None,
                                                    memory_mb: None,
                                                    uptime: None,
                                                    port: def.port,
                                                    tunnel_url: None,
                                                    cwd: default_cwd,
                                                    config_path: Some(config_path.clone()),
                                                });
                                            }
                                        }
                                        continue;
                                    }
                                }

                                // Fallback if no valid config file exists: show only alive processes
                                let mut s_names: Vec<String> =
                                    state.processes.keys().cloned().collect();
                                s_names.sort();

                                for service_name in &s_names {
                                    let proc_entry = state.processes.get(service_name).unwrap();
                                    if is_alive(proc_entry.pid) {
                                        let (cpu, memory_mb) = metrics.query_tree(proc_entry.pid);
                                        let tunnel = state.tunnels.get(service_name);
                                        let tunnel_url =
                                            if tunnel.map(|t| is_alive(t.pid)).unwrap_or(false) {
                                                tunnel.and_then(|t| t.url.clone())
                                            } else {
                                                None
                                            };

                                        rows.push(GlobalProcRow {
                                            project_key: dir_key.clone(),
                                            service_name: service_name.clone(),
                                            running: true,
                                            pid: Some(proc_entry.pid),
                                            cpu,
                                            memory_mb,
                                            uptime: Some(uptime_string(&proc_entry.started_at)),
                                            port: proc_entry.port,
                                            tunnel_url,
                                            cwd: proc_entry.cwd.clone(),
                                            config_path: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Deterministic sort: group by project_key then service_name
    rows.sort_by(|a, b| {
        a.project_key
            .cmp(&b.project_key)
            .then_with(|| a.service_name.cmp(&b.service_name))
    });

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config::ProcessDef;
    use std::collections::HashMap;

    fn make_process_def(depends_on: Vec<&str>) -> ProcessDef {
        ProcessDef {
            cmd: "echo test".to_string(),
            cwd: None,
            env: HashMap::new(),
            port: None,
            forward: false,
            tunnel: false,
            free_port: false,
            log_file: None,
            depends_on: depends_on.into_iter().map(|s| s.to_string()).collect(),
            watch: None,
            watch_ignore: vec![],
        }
    }

    #[test]
    fn test_topological_sort_linear() {
        let mut processes = HashMap::new();
        processes.insert("web".to_string(), make_process_def(vec!["api"]));
        processes.insert("api".to_string(), make_process_def(vec!["db"]));
        processes.insert("db".to_string(), make_process_def(vec![]));

        let config = Config { processes };
        let order = resolve_dependency_order(&config, None).unwrap();

        let db_pos = order.iter().position(|x| x == "db").unwrap();
        let api_pos = order.iter().position(|x| x == "api").unwrap();
        let web_pos = order.iter().position(|x| x == "web").unwrap();

        assert!(db_pos < api_pos);
        assert!(api_pos < web_pos);
    }

    #[test]
    fn test_topological_sort_single_target() {
        let mut processes = HashMap::new();
        processes.insert("web".to_string(), make_process_def(vec!["api"]));
        processes.insert("api".to_string(), make_process_def(vec!["db"]));
        processes.insert("db".to_string(), make_process_def(vec![]));
        processes.insert("isolated_worker".to_string(), make_process_def(vec![]));

        let config = Config { processes };
        let order = resolve_dependency_order(&config, Some("web")).unwrap();

        assert_eq!(order, vec!["db", "api", "web"]);
        assert!(!order.contains(&"isolated_worker".to_string()));
    }

    #[test]
    fn test_topological_sort_diamond() {
        let mut processes = HashMap::new();
        processes.insert("web".to_string(), make_process_def(vec!["api1", "api2"]));
        processes.insert("api1".to_string(), make_process_def(vec!["db"]));
        processes.insert("api2".to_string(), make_process_def(vec!["db"]));
        processes.insert("db".to_string(), make_process_def(vec![]));

        let config = Config { processes };
        let order = resolve_dependency_order(&config, None).unwrap();

        let db_pos = order.iter().position(|x| x == "db").unwrap();
        let api1_pos = order.iter().position(|x| x == "api1").unwrap();
        let api2_pos = order.iter().position(|x| x == "api2").unwrap();
        let web_pos = order.iter().position(|x| x == "web").unwrap();

        assert!(db_pos < api1_pos);
        assert!(db_pos < api2_pos);
        assert!(api1_pos < web_pos);
        assert!(api2_pos < web_pos);
    }

    #[test]
    fn test_cycle_detection() {
        let mut processes = HashMap::new();
        processes.insert("a".to_string(), make_process_def(vec!["b"]));
        processes.insert("b".to_string(), make_process_def(vec!["a"]));

        let config = Config { processes };
        let result = resolve_dependency_order(&config, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Cyclic dependency detected"));
    }

    #[test]
    fn test_unknown_dependency() {
        let mut processes = HashMap::new();
        processes.insert("web".to_string(), make_process_def(vec!["non_existent"]));

        let config = Config { processes };
        let result = resolve_dependency_order(&config, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unknown dependency \"non_existent\""));
    }

    #[test]
    fn test_state_serialization_with_config_path() {
        let mut state = State::default();
        state.config_path = Some(PathBuf::from("/tmp/my-project/procman.yaml"));
        let json = serde_json::to_string(&state).unwrap();
        let loaded: State = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded.config_path,
            Some(PathBuf::from("/tmp/my-project/procman.yaml"))
        );
    }
}
