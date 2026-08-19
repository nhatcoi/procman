use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use regex::Regex;
use std::fs::{self, OpenOptions};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::paths::project_log_dir;
use crate::process_manager::stop_by_pid;
use crate::state::{is_alive, read_state, write_state, TunnelEntry};

const CLOUDFLARED_BIN: &str = "cloudflared";
const QUICK_TUNNEL_URL_REGEX: &str = r"https://[a-zA-Z0-9-]+\.trycloudflare\.com";
const TUNNEL_WAIT_TIMEOUT: Duration = Duration::from_secs(25);
const TUNNEL_POLL_INTERVAL: Duration = Duration::from_millis(300);
const TUNNEL_LOG_SUFFIX: &str = "tunnel.log";

pub fn cloudflared_available() -> bool {
    Command::new(CLOUDFLARED_BIN)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn tunnel_log_file_for(config_path: &Path, name: &str) -> Result<PathBuf> {
    Ok(project_log_dir(config_path)?.join(format!("{}.{}", name, TUNNEL_LOG_SUFFIX)))
}

fn wait_for_url(log_file: &Path, timeout: Duration) -> Option<String> {
    let re = Regex::new(QUICK_TUNNEL_URL_REGEX).ok()?;
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if log_file.is_file() {
            if let Ok(content) = fs::read_to_string(log_file) {
                if let Some(m) = re.find(&content) {
                    return Some(m.as_str().to_string());
                }
            }
        }
        thread::sleep(TUNNEL_POLL_INTERVAL);
    }
    None
}

pub fn forward_start(config_path: &Path, config: &Config, name: &str) -> Result<(String, i32)> {
    let def = config
        .processes
        .get(name)
        .ok_or_else(|| anyhow!("Unknown process \"{}\" (check procman.yaml)", name))?;

    if !cloudflared_available() {
        return Err(anyhow!(
            "cloudflared not found on PATH. Install it: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/"
        ));
    }

    let mut state = read_state(config_path);
    if let Some(existing) = state.tunnels.get(name) {
        if is_alive(existing.pid) {
            if let Some(ref url) = existing.url {
                return Ok((url.clone(), existing.pid));
            }
        }
    }

    let port = state
        .processes
        .get(name)
        .and_then(|p| p.port)
        .or(def.port)
        .ok_or_else(|| {
            anyhow!(
                "No \"port\" configured for process \"{}\" in procman.yaml — required to forward it.",
                name
            )
        })?;

    let log_file = tunnel_log_file_for(config_path, name)?;
    let header = format!(
        "----- procman forward: {} -> localhost:{} @ {} -----\n",
        name,
        port,
        Utc::now().to_rfc3339()
    );
    fs::write(&log_file, header)?;

    let log_fd = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .with_context(|| format!("Failed to open tunnel log file {:?}", log_file))?;

    let log_fd_err = log_fd
        .try_clone()
        .with_context(|| "Failed to clone file descriptor for tunnel log")?;

    let mut cmd = Command::new(CLOUDFLARED_BIN);
    cmd.args(["tunnel", "--url", &format!("http://localhost:{}", port)])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_fd))
        .stderr(Stdio::from(log_fd_err));

    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .with_context(|| "Failed to spawn cloudflared tunnel")?;
    let child_pid = child.id() as i32;

    // Fast check if URL is already available in 1 pass
    let immediate_url = if let Ok(content) = fs::read_to_string(&log_file) {
        Regex::new(QUICK_TUNNEL_URL_REGEX)
            .ok()
            .and_then(|re| re.find(&content).map(|m| m.as_str().to_string()))
    } else {
        None
    };

    state.tunnels.insert(
        name.to_string(),
        TunnelEntry {
            pid: child_pid,
            port,
            url: immediate_url.clone(),
            log_file: log_file.to_string_lossy().to_string(),
            started_at: Utc::now().to_rfc3339(),
        },
    );

    write_state(config_path, &state)?;

    // Spawn background poller if URL not yet found
    if immediate_url.is_none() {
        let log_file_clone = log_file.clone();
        let cp_clone = config_path.to_path_buf();
        let name_clone = name.to_string();
        thread::spawn(move || {
            if let Some(discovered_url) = wait_for_url(&log_file_clone, TUNNEL_WAIT_TIMEOUT) {
                let mut st = read_state(&cp_clone);
                if let Some(t) = st.tunnels.get_mut(&name_clone) {
                    if t.pid == child_pid {
                        t.url = Some(discovered_url);
                        let _ = write_state(&cp_clone, &st);
                    }
                }
            }
        });
    }

    Ok((
        immediate_url.unwrap_or_else(|| "initializing tunnel...".to_string()),
        child_pid,
    ))
}

pub fn forward_stop(config_path: &Path, name: &str) -> Result<bool> {
    let mut state = read_state(config_path);
    if let Some(tunnel) = state.tunnels.remove(name) {
        if is_alive(tunnel.pid) {
            let _ = stop_by_pid(tunnel.pid);
        }
        write_state(config_path, &state)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
