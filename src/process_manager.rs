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

const SIGTERM_TIMEOUT: Duration = Duration::from_millis(1000);
const SIGKILL_TIMEOUT: Duration = Duration::from_millis(800);
const POLL_INTERVAL: Duration = Duration::from_millis(30);
const SHELL_BIN: &str = "sh";
const LOG_FILE_EXTENSION: &str = "log";

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
        .arg(format!(
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

    let should_free_port =
        force || def.free_port.unwrap_or(false) || def.kill_before_run.unwrap_or(false);

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

pub fn stop(config_path: &Path, config: &Config, name: Option<&str>) -> Result<Vec<StatusRow>> {
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

fn uptime_string(started_at_str: &str) -> String {
    let started_at = DateTime::parse_from_rfc3339(started_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let dur = Utc::now().signed_duration_since(started_at);
    let secs = dur.num_seconds().max(0);
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn status(config_path: &Path, config: &Config, name: Option<&str>) -> Result<Vec<StatusRow>> {
    let mut metrics = ProcessMetrics::new();
    status_with_metrics(config_path, config, name, &mut metrics)
}

pub fn status_with_metrics(
    config_path: &Path,
    config: &Config,
    name: Option<&str>,
    metrics: &mut ProcessMetrics,
) -> Result<Vec<StatusRow>> {
    let _ = crate::registry::ProjectRegistry::register(config_path);
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

#[derive(Debug, Clone)]
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
    let mut metrics = ProcessMetrics::new();
    scan_global_processes_with_metrics(&mut metrics)
}

pub fn scan_global_processes_with_metrics(
    metrics: &mut ProcessMetrics,
) -> Result<Vec<GlobalProcRow>> {
    let mut rows = Vec::new();
    let mut processed_keys = std::collections::HashSet::new();

    // 1. Scan from Known Projects Registry
    let registry = crate::registry::ProjectRegistry::load();
    for proj in registry.get_all() {
        if !proj.config_path.is_file() {
            continue;
        }
        processed_keys.insert(proj.project_key.clone());

        if let Ok(cfg) = crate::config::load_config(&proj.config_path) {
            let state = read_state(&proj.config_path);
            for (service_name, def) in &cfg.processes {
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

    // 2. Scan active process states in ~/.local/state/procman/ that might not be in registry
    let state_root = crate::paths::global_state_root();
    if state_root.is_dir() {
        if let Ok(entries) = fs::read_dir(&state_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let project_key = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    if processed_keys.contains(&project_key) {
                        continue;
                    }

                    let state_file = path.join("state.json");
                    if state_file.is_file() {
                        if let Ok(content) = fs::read_to_string(&state_file) {
                            if let Ok(state) = serde_json::from_str::<State>(&content) {
                                for (service_name, proc_entry) in &state.processes {
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
                                            project_key: project_key.clone(),
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

    rows.sort_by(|a, b| {
        a.project_key
            .cmp(&b.project_key)
            .then_with(|| a.service_name.cmp(&b.service_name))
    });

    Ok(rows)
}
