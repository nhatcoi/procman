use std::collections::HashSet;

use crate::engine::supervisor::{GlobalProcRow, StatusRow};

const STATUS_UP: &str = "up";
const STATUS_DOWN: &str = "down";
const EMPTY_PLACEHOLDER: &str = "-";

pub fn print_status_table(rows: &[StatusRow]) {
    if rows.is_empty() {
        println!("No processes configured.");
        return;
    }

    let mut name_w = "NAME".len();
    for r in rows {
        name_w = name_w.max(r.name.len());
    }

    println!(
        "{:<name_w$}  {:<7}  {:<7}  {:<7}  {:<7}  {:<7}  {:<5}  TUNNEL",
        "NAME",
        "STATUS",
        "PID",
        "CPU",
        "MEM",
        "UPTIME",
        "PORT",
        name_w = name_w
    );

    for r in rows {
        let status = if r.running { STATUS_UP } else { STATUS_DOWN };
        let pid = r
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let cpu = r
            .cpu
            .map(|c| format!("{:.1}%", c))
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let mem = r
            .memory_mb
            .map(|m| format!("{}MB", m))
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let uptime = r.uptime.as_deref().unwrap_or(EMPTY_PLACEHOLDER);
        let port = r
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let tunnel = r.tunnel_url.as_deref().unwrap_or(EMPTY_PLACEHOLDER);

        println!(
            "{:<name_w$}  {:<7}  {:<7}  {:<7}  {:<7}  {:<7}  {:<5}  {}",
            r.name,
            status,
            pid,
            cpu,
            mem,
            uptime,
            port,
            tunnel,
            name_w = name_w
        );
    }
}

pub fn print_global_processes(rows: &[GlobalProcRow]) {
    if rows.is_empty() {
        println!("No active procman processes found on this system.");
        return;
    }

    let mut proj_w = "PROJECT".len();
    let mut name_w = "SERVICE".len();
    let mut distinct_projects = HashSet::new();

    for r in rows {
        proj_w = proj_w.max(r.project_key.len());
        name_w = name_w.max(r.service_name.len());
        distinct_projects.insert(&r.project_key);
    }

    println!(
        "{:<proj_w$}  {:<name_w$}  {:<7}  {:<7}  {:<7}  {:<7}  {:<5}  {:<35}  CWD",
        "PROJECT",
        "SERVICE",
        "PID",
        "CPU",
        "MEM",
        "UPTIME",
        "PORT",
        "TUNNEL",
        proj_w = proj_w,
        name_w = name_w
    );

    for r in rows {
        let cpu = r
            .cpu
            .map(|c| format!("{:.1}%", c))
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let mem = r
            .memory_mb
            .map(|m| format!("{}MB", m))
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let pid_str = r
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let uptime = r.uptime.as_deref().unwrap_or(EMPTY_PLACEHOLDER);
        let port = r
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| EMPTY_PLACEHOLDER.to_string());
        let tunnel = r.tunnel_url.as_deref().unwrap_or(EMPTY_PLACEHOLDER);

        println!(
            "{:<proj_w$}  {:<name_w$}  {:<7}  {:<7}  {:<7}  {:<7}  {:<5}  {:<35}  {}",
            r.project_key,
            r.service_name,
            pid_str,
            cpu,
            mem,
            uptime,
            port,
            tunnel,
            r.cwd,
            proj_w = proj_w,
            name_w = name_w
        );
    }

    println!(
        "\nTotal: {} active process(es) across {} project(s).",
        rows.len(),
        distinct_projects.len()
    );
}
