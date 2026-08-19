use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::cloudflare::forward_start;
use crate::config::{Config, ProcessDef};
use crate::metrics::ProcessMetrics;
use crate::paths::project_log_dir;
use crate::state::{is_alive, read_state, write_state, ProcEntry, State};

const SIGTERM_TIMEOUT: Duration = Duration::from_millis(5000);
const SIGKILL_TIMEOUT: Duration = Duration::from_millis(2000);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHELL_BIN: &str = "sh";
const LOG_FILE_EXTENSION: &str = "log";
const DEFAULT_PLACEHOLDER: &str = "-";

#[derive(Debug, Clone)]
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

pub fn log_file_for(config_path: &Path, name: &str) -> Result<PathBuf> {
    Ok(project_log_dir(config_path)?.join(format!("{}.{}", name, LOG_FILE_EXTENSION)))
}

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
        .arg(&format!(
            "lsof -ti :{} 2>/dev/null | xargs -r kill -9 2>/dev/null",
            port
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

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

pub fn force_kill_by_pid(pid: i32) -> Result<()> {
    if pid <= 0 {
        return Ok(());
    }
    let _ = kill(Pid::from_raw(-pid), Signal::SIGKILL);
    let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
    Ok(())
}

pub fn start_one(
    config_path: &Path,
    name: &str,
    def: &ProcessDef,
    state: &mut State,
    force: bool,
) -> Result<ProcEntry> {
    if let Some(existing) = state.processes.get(name) {
        if is_alive(existing.pid) {
            return Ok(existing.clone());
        }
    }

    let should_free_port = force
        || def.free_port.unwrap_or(false)
        || def.kill_before_run.unwrap_or(false);

    if should_free_port {
        if let Some(port) = def.port {
            kill_port(port);
        }
    }

    let config_dir = config_path.parent().unwrap_or(config_path);
    let cwd = match &def.cwd {
        Some(c) => config_dir.join(c),
        None => config_dir.to_path_buf(),
    };
    let abs_cwd = fs::canonicalize(&cwd).unwrap_or(cwd);

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
    let _ = log_fd.write_all(header.as_bytes());

    let log_fd_out = OpenOptions::new().append(true).open(&log_file)?;
    let log_fd_err = OpenOptions::new().append(true).open(&log_file)?;

    let mut cmd = Command::new(SHELL_BIN);
    cmd.arg("-c")
        .arg(&def.cmd)
        .current_dir(&abs_cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_fd_out))
        .stderr(Stdio::from(log_fd_err));

    if let Some(ref env_vars) = def.env {
        for (k, v) in env_vars {
            cmd.env(k, v);
        }
    }

    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn process \"{}\"", name))?;
    let child_pid = child.id() as i32;

    let entry = ProcEntry {
        pid: child_pid,
        cmd: def.cmd.clone(),
        cwd: abs_cwd.to_string_lossy().to_string(),
        log_file: log_file.to_string_lossy().to_string(),
        port: def.port,
        started_at: Utc::now().to_rfc3339(),
    };

    state.processes.insert(name.to_string(), entry.clone());
    Ok(entry)
}

pub fn start(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
    force: bool,
) -> Result<Vec<StatusRow>> {
    let mut state = read_state(config_path);
    let names = resolve_names(config, name)?;

    for n in &names {
        if let Some(def) = config.processes.get(n) {
            start_one(config_path, n, def, &mut state, force)?;
        }
    }
    write_state(config_path, &state)?;

    for n in &names {
        if let Some(def) = config.processes.get(n) {
            if def.forward.unwrap_or(false) || def.tunnel.unwrap_or(false) {
                if let Err(err) = forward_start(config_path, config, n) {
                    eprintln!("[procman] tunnel warning for \"{}\": {}", n, err);
                }
            }
        }
    }

    status(config_path, config, name)
}

pub fn stop(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
) -> Result<Vec<StatusRow>> {
    let mut state = read_state(config_path);
    let names = resolve_names(config, name)?;

    for n in &names {
        if let Some(entry) = state.processes.remove(n) {
            if is_alive(entry.pid) {
                let _ = stop_by_pid(entry.pid);
            }
        }
        if let Some(tunnel) = state.tunnels.remove(n) {
            if is_alive(tunnel.pid) {
                let _ = stop_by_pid(tunnel.pid);
            }
        }
    }
    write_state(config_path, &state)?;
    status(config_path, config, name)
}

pub fn force_stop(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
) -> Result<Vec<StatusRow>> {
    let mut state = read_state(config_path);
    let names = resolve_names(config, name)?;

    for n in &names {
        if let Some(def) = config.processes.get(n) {
            if let Some(port) = def.port {
                kill_port(port);
            }
        }
        if let Some(entry) = state.processes.remove(n) {
            let _ = force_kill_by_pid(entry.pid);
        }
        if let Some(tunnel) = state.tunnels.remove(n) {
            let _ = force_kill_by_pid(tunnel.pid);
        }
    }
    write_state(config_path, &state)?;
    status(config_path, config, name)
}

pub fn restart(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
    force: bool,
) -> Result<Vec<StatusRow>> {
    if force {
        force_stop(config_path, config, name)?;
    } else {
        stop(config_path, config, name)?;
    }
    start(config_path, config, name, force)
}

fn uptime_string(started_at: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(started_at) {
        let diff = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
        let total_seconds = diff.num_seconds();
        if total_seconds < 60 {
            return format!("{}s", total_seconds);
        }
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        if minutes < 60 {
            return format!("{}m{}s", minutes, seconds);
        }
        let hours = minutes / 60;
        let rem_minutes = minutes % 60;
        return format!("{}h{}m", hours, rem_minutes);
    }
    DEFAULT_PLACEHOLDER.to_string()
}

pub fn status(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
) -> Result<Vec<StatusRow>> {
    let state = read_state(config_path);
    let names = resolve_names(config, name)?;
    let mut metrics = ProcessMetrics::new();

    let mut rows = Vec::with_capacity(names.len());
    for n in names {
        let entry = state.processes.get(&n);
        let def = match config.processes.get(&n) {
            Some(d) => d,
            None => continue,
        };

        let running = entry.map(|e| is_alive(e.pid)).unwrap_or(false);
        let (cpu, memory_mb) = if running {
            if let Some(e) = entry {
                metrics.query(e.pid)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let tunnel = state.tunnels.get(&n);
        let tunnel_alive = tunnel.map(|t| is_alive(t.pid)).unwrap_or(false);

        let default_log = log_file_for(config_path, &n)?
            .to_string_lossy()
            .to_string();

        rows.push(StatusRow {
            name: n.clone(),
            running,
            pid: if running { entry.map(|e| e.pid) } else { None },
            cpu,
            memory_mb,
            uptime: if running {
                entry.map(|e| uptime_string(&e.started_at))
            } else {
                None
            },
            port: entry.and_then(|e| e.port).or(def.port),
            log_file: entry.map(|e| e.log_file.clone()).unwrap_or(default_log),
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
        });
    }

    Ok(rows)
}

#[derive(Debug, Clone)]
pub struct GlobalProcRow {
    pub project_key: String,
    pub service_name: String,
    pub pid: i32,
    pub cpu: Option<f32>,
    pub memory_mb: Option<u64>,
    pub uptime: Option<String>,
    pub port: Option<u16>,
    pub tunnel_url: Option<String>,
    pub cwd: String,
}

pub fn scan_global_processes() -> Result<Vec<GlobalProcRow>> {
    let state_root = crate::paths::global_state_root();
    if !state_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut metrics = ProcessMetrics::new();
    let mut rows = Vec::new();

    let entries = fs::read_dir(&state_root)
        .with_context(|| format!("Failed to read directory {:?}", state_root))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let state_file = path.join("state.json");
            if state_file.is_file() {
                if let Ok(content) = fs::read_to_string(&state_file) {
                    if let Ok(state) = serde_json::from_str::<State>(&content) {
                        let project_key = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();

                        for (service_name, proc_entry) in &state.processes {
                            if is_alive(proc_entry.pid) {
                                let (cpu, memory_mb) = metrics.query(proc_entry.pid);
                                let tunnel = state.tunnels.get(service_name);
                                let tunnel_url = if tunnel.map(|t| is_alive(t.pid)).unwrap_or(false) {
                                    tunnel.and_then(|t| t.url.clone())
                                } else {
                                    None
                                };

                                rows.push(GlobalProcRow {
                                    project_key: project_key.clone(),
                                    service_name: service_name.clone(),
                                    pid: proc_entry.pid,
                                    cpu,
                                    memory_mb,
                                    uptime: Some(uptime_string(&proc_entry.started_at)),
                                    port: proc_entry.port,
                                    tunnel_url,
                                    cwd: proc_entry.cwd.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    rows.sort_by(|a, b| {
        a.project_key
            .cmp(&b.project_key)
            .then_with(|| a.service_name.cmp(&b.service_name))
    });

    Ok(rows)
}
