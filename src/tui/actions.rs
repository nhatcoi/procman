use std::sync::Arc;
use std::thread;

use super::app::AppState;
use crate::engine::config::load_config;
use crate::engine::registry::ProjectRegistry;
use crate::engine::supervisor::{self, force_kill_by_pid, stop_by_pid};
use crate::tunnels::cloudflare::{forward_start, forward_stop};

pub fn start_selected(state: &mut AppState) {
    if state.is_global_mode {
        if let Some(g) = state.global_rows.get(state.selected_idx).cloned() {
            if let Some(ref cp) = g.config_path {
                let key = format!("{}/{}", g.project_key, g.service_name);
                if let Ok(mut pending) = state.pending_actions.lock() {
                    pending.insert(key, "starting...".to_string());
                }
                state.status_msg = format!("⟳ starting {} in {}...", g.service_name, g.project_key);

                let cp_clone = cp.clone();
                let service_clone = g.service_name.clone();
                let pending_clone = Arc::clone(&state.pending_actions);
                let key_clone = format!("{}/{}", g.project_key, g.service_name);

                thread::spawn(move || {
                    if let Ok(cfg) = load_config(&cp_clone) {
                        let _ = supervisor::start(&cp_clone, &cfg, Some(&service_clone), false);
                    }
                    if let Ok(mut p) = pending_clone.lock() {
                        p.remove(&key_clone);
                    }
                });
            }
        }
    } else if let Some((ref cp, ref cfg)) = state.local_config {
        if let Some(target_name) = state
            .local_rows
            .get(state.selected_idx)
            .map(|r| r.name.clone())
        {
            if let Ok(mut pending) = state.pending_actions.lock() {
                pending.insert(target_name.clone(), "starting...".to_string());
            }
            state.status_msg = format!("⟳ starting {}...", target_name);

            let cp_clone = cp.clone();
            let cfg_clone = cfg.clone();
            let target_clone = target_name.clone();
            let pending_clone = Arc::clone(&state.pending_actions);

            thread::spawn(move || {
                let _ = supervisor::start(&cp_clone, &cfg_clone, Some(&target_clone), false);
                if let Ok(mut p) = pending_clone.lock() {
                    p.remove(&target_clone);
                }
            });
        }
    }
}

pub fn restart_selected(state: &mut AppState) {
    if state.is_global_mode {
        if let Some(g) = state.global_rows.get(state.selected_idx).cloned() {
            if let Some(ref cp) = g.config_path {
                let key = format!("{}/{}", g.project_key, g.service_name);
                if let Ok(mut pending) = state.pending_actions.lock() {
                    pending.insert(key, "restarting...".to_string());
                }
                state.status_msg =
                    format!("⟳ restarting {} in {}...", g.service_name, g.project_key);

                let cp_clone = cp.clone();
                let service_clone = g.service_name.clone();
                let pending_clone = Arc::clone(&state.pending_actions);
                let key_clone = format!("{}/{}", g.project_key, g.service_name);

                thread::spawn(move || {
                    if let Ok(cfg) = load_config(&cp_clone) {
                        let _ = supervisor::restart(&cp_clone, &cfg, Some(&service_clone), false);
                    }
                    if let Ok(mut p) = pending_clone.lock() {
                        p.remove(&key_clone);
                    }
                });
            }
        }
    } else if let Some((ref cp, ref cfg)) = state.local_config {
        if let Some(target_name) = state
            .local_rows
            .get(state.selected_idx)
            .map(|r| r.name.clone())
        {
            if let Ok(mut pending) = state.pending_actions.lock() {
                pending.insert(target_name.clone(), "restarting...".to_string());
            }
            state.status_msg = format!("⟳ restarting {}...", target_name);

            let cp_clone = cp.clone();
            let cfg_clone = cfg.clone();
            let target_clone = target_name.clone();
            let pending_clone = Arc::clone(&state.pending_actions);

            thread::spawn(move || {
                let _ = supervisor::restart(&cp_clone, &cfg_clone, Some(&target_clone), false);
                if let Ok(mut p) = pending_clone.lock() {
                    p.remove(&target_clone);
                }
            });
        }
    }
}

pub fn stop_selected(state: &mut AppState) {
    if state.is_global_mode {
        if let Some(g) = state.global_rows.get(state.selected_idx).cloned() {
            let key = format!("{}/{}", g.project_key, g.service_name);
            if let Ok(mut pending) = state.pending_actions.lock() {
                pending.insert(key, "stopping...".to_string());
            }
            state.status_msg = format!("⟳ stopping {} in {}...", g.service_name, g.project_key);

            let cp_opt = g.config_path.clone();
            let service_clone = g.service_name.clone();
            let pid_opt = g.pid;
            let pending_clone = Arc::clone(&state.pending_actions);
            let key_clone = format!("{}/{}", g.project_key, g.service_name);

            thread::spawn(move || {
                if let Some(cp) = cp_opt {
                    if let Ok(cfg) = load_config(&cp) {
                        let _ = supervisor::stop(&cp, &cfg, Some(&service_clone));
                    } else if let Some(pid) = pid_opt {
                        let _ = stop_by_pid(pid);
                    }
                } else if let Some(pid) = pid_opt {
                    let _ = stop_by_pid(pid);
                }
                if let Ok(mut p) = pending_clone.lock() {
                    p.remove(&key_clone);
                }
            });
        }
    } else if let Some((ref cp, ref cfg)) = state.local_config {
        if let Some(target_name) = state
            .local_rows
            .get(state.selected_idx)
            .map(|r| r.name.clone())
        {
            if let Ok(mut pending) = state.pending_actions.lock() {
                pending.insert(target_name.clone(), "stopping...".to_string());
            }
            state.status_msg = format!("⟳ stopping {}...", target_name);

            let cp_clone = cp.clone();
            let cfg_clone = cfg.clone();
            let target_clone = target_name.clone();
            let pending_clone = Arc::clone(&state.pending_actions);

            thread::spawn(move || {
                let _ = supervisor::stop(&cp_clone, &cfg_clone, Some(&target_clone));
                if let Ok(mut p) = pending_clone.lock() {
                    p.remove(&target_clone);
                }
            });
        }
    }
}

pub fn force_kill_selected(state: &mut AppState) {
    if state.is_global_mode {
        if let Some(g) = state.global_rows.get(state.selected_idx).cloned() {
            let key = format!("{}/{}", g.project_key, g.service_name);
            if let Ok(mut pending) = state.pending_actions.lock() {
                pending.insert(key, "killing...".to_string());
            }
            state.status_msg = format!(
                "🛑 force-killing {} in {}...",
                g.service_name, g.project_key
            );

            let cp_opt = g.config_path.clone();
            let service_clone = g.service_name.clone();
            let pid_opt = g.pid;
            let port_opt = g.port;
            let pending_clone = Arc::clone(&state.pending_actions);
            let key_clone = format!("{}/{}", g.project_key, g.service_name);

            thread::spawn(move || {
                if let Some(cp) = cp_opt {
                    if let Ok(cfg) = load_config(&cp) {
                        let _ = supervisor::force_stop(&cp, &cfg, Some(&service_clone));
                    } else if let Some(pid) = pid_opt {
                        let _ = force_kill_by_pid(pid);
                        if let Some(port) = port_opt {
                            supervisor::kill_port(port);
                        }
                    }
                } else if let Some(pid) = pid_opt {
                    let _ = force_kill_by_pid(pid);
                    if let Some(port) = port_opt {
                        supervisor::kill_port(port);
                    }
                }
                if let Ok(mut p) = pending_clone.lock() {
                    p.remove(&key_clone);
                }
            });
        }
    } else if let Some((ref cp, ref cfg)) = state.local_config {
        if let Some(target_name) = state
            .local_rows
            .get(state.selected_idx)
            .map(|r| r.name.clone())
        {
            if let Ok(mut pending) = state.pending_actions.lock() {
                pending.insert(target_name.clone(), "killing...".to_string());
            }
            state.status_msg = format!("🛑 force-killing {}...", target_name);

            let cp_clone = cp.clone();
            let cfg_clone = cfg.clone();
            let target_clone = target_name.clone();
            let pending_clone = Arc::clone(&state.pending_actions);

            thread::spawn(move || {
                let _ = supervisor::force_stop(&cp_clone, &cfg_clone, Some(&target_clone));
                if let Ok(mut p) = pending_clone.lock() {
                    p.remove(&target_clone);
                }
            });
        }
    }
}

pub fn forward_selected(state: &mut AppState) {
    if state.is_global_mode {
        if let Some(g) = state.global_rows.get(state.selected_idx).cloned() {
            if let Some(ref cp) = g.config_path {
                if let Ok(cfg) = load_config(cp) {
                    match forward_start(cp, &cfg, &g.service_name) {
                        Ok((url, _)) => {
                            state.status_msg = format!("{} -> {}", g.service_name, url);
                        }
                        Err(e) => {
                            state.status_msg = format!("forward failed: {}", e);
                        }
                    }
                }
            }
        }
    } else if let Some((ref cp, ref cfg)) = state.local_config {
        if let Some(target_name) = state
            .local_rows
            .get(state.selected_idx)
            .map(|r| r.name.clone())
        {
            match forward_start(cp, cfg, &target_name) {
                Ok((url, _)) => {
                    state.status_msg = format!("{} -> {}", target_name, url);
                }
                Err(e) => {
                    state.status_msg = format!("forward failed: {}", e);
                }
            }
        }
    }
}

pub fn unforward_selected(state: &mut AppState) {
    if state.is_global_mode {
        if let Some(g) = state.global_rows.get(state.selected_idx).cloned() {
            if let Some(ref cp) = g.config_path {
                match forward_stop(cp, &g.service_name) {
                    Ok(stopped) => {
                        state.status_msg = if stopped {
                            format!("tunnel stopped for {}", g.service_name)
                        } else {
                            format!("no tunnel for {}", g.service_name)
                        };
                    }
                    Err(e) => {
                        state.status_msg = format!("error stopping tunnel: {}", e);
                    }
                }
            }
        }
    } else if let Some((ref cp, _)) = state.local_config {
        if let Some(target_name) = state
            .local_rows
            .get(state.selected_idx)
            .map(|r| r.name.clone())
        {
            match forward_stop(cp, &target_name) {
                Ok(stopped) => {
                    state.status_msg = if stopped {
                        format!("tunnel stopped for {}", target_name)
                    } else {
                        format!("no tunnel for {}", target_name)
                    };
                }
                Err(e) => {
                    state.status_msg = format!("error stopping tunnel: {}", e);
                }
            }
        }
    }
}

pub fn unregister_selected(state: &mut AppState) {
    if state.is_global_mode {
        if let Some(g) = state.global_rows.get(state.selected_idx).cloned() {
            let _ = ProjectRegistry::unregister(&g.project_key);
            state.refresh_rows();
            state.status_msg = format!("unregistered project \"{}\" from dashboard", g.project_key);
        }
    }
}
