use anyhow::Result;
use clap::{Parser, Subcommand};

mod cloudflare;
mod config;
mod logs;
mod metrics;
mod paths;
mod process_manager;
mod qr;
mod state;
mod ui;
pub mod uninstaller;
pub mod updater;

use config::require_config;
use process_manager::{GlobalProcRow, StatusRow};

const APP_NAME: &str = "procman";
const APP_AUTHOR: &str = "nhatcoi";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_ABOUT: &str =
    "Custom process manager: start/stop/log/forward processes from a project config file";

const STATUS_UP: &str = "up";
const STATUS_DOWN: &str = "down";
const EMPTY_PLACEHOLDER: &str = "-";
const DEFAULT_LOG_TAIL_LINES: usize = 100;

#[derive(Parser, Debug)]
#[command(
    name = APP_NAME,
    author = APP_AUTHOR,
    version = APP_VERSION,
    about = APP_ABOUT
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start one process, or all processes if no name is given
    Start {
        name: Option<String>,
        #[arg(short, long)]
        force: bool,
    },
    /// Stop one process, or all processes if no name is given
    Stop { name: Option<String> },
    /// Force kill one or all processes (sends SIGKILL and frees ports)
    Kill { name: Option<String> },
    /// Force kill whatever process is occupying a specific port
    KillPort { port: u16 },
    /// Restart one process, or all processes if no name is given
    Restart {
        name: Option<String>,
        #[arg(short, long)]
        force: bool,
    },
    /// Show status of all processes in the current project (default command)
    Status { name: Option<String> },
    /// List all active processes across all projects on the system
    #[command(alias = "ls", alias = "list")]
    Ps,
    /// Show logs for a process
    Logs {
        name: String,
        #[arg(short, long)]
        follow: bool,
        #[arg(short = 'n', long, default_value_t = DEFAULT_LOG_TAIL_LINES)]
        lines: usize,
    },
    /// Expose a process's port via a cloudflared quick tunnel
    Forward { name: String },
    /// Stop the cloudflared tunnel for a process
    ForwardStop { name: String },
    /// Print ASCII QR code for mobile scanning
    Qr { name: String },
    /// Upgrade procman to the latest available release
    #[command(alias = "update", alias = "self-update")]
    Upgrade {
        /// Automatically accept upgrade prompt without asking
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Uninstall procman completely from your system
    #[command(alias = "self-uninstall")]
    Uninstall {
        /// Automatically accept confirmation prompt without asking
        #[arg(short = 'y', long)]
        yes: bool,
        /// Purge all procman state and logs directories (~/.local/state/procman, ~/.local/share/procman)
        #[arg(short = 'p', long)]
        purge: bool,
    },
    /// Install procman agent skill (.agents/skills/procman/SKILL.md) into current project
    #[command(alias = "install-skill", alias = "init-skill")]
    Skill {
        /// Custom destination path (default: .agents/skills/procman/SKILL.md)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Interactive dashboard (start/stop/logs/forward/fullscreen/search)
    Ui {
        /// Launch directly into Global Dashboard showing all projects across the system
        #[arg(short = 'a', long = "all", short_alias = 'g', alias = "global")]
        all: bool,
    },
}

fn print_status(rows: &[StatusRow]) {
    if rows.is_empty() {
        println!("No processes defined.");
        return;
    }

    let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);

    let header = format!(
        "{:<name_w$}  {:<7}  {:<7}  {:<7}  {:<7}  {:<7}  {:<5}  {}",
        "NAME",
        "STATUS",
        "PID",
        "CPU",
        "MEM",
        "UPTIME",
        "PORT",
        "TUNNEL",
        name_w = name_w
    );
    println!("{}", header);

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

fn print_global_ps(rows: &[GlobalProcRow]) {
    if rows.is_empty() {
        println!("No active procman processes running across the system.");
        return;
    }

    let proj_w = rows
        .iter()
        .map(|r| r.project_key.len())
        .max()
        .unwrap_or(7)
        .max(7);

    let name_w = rows
        .iter()
        .map(|r| r.service_name.len())
        .max()
        .unwrap_or(7)
        .max(7);

    let header = format!(
        "{:<proj_w$}  {:<name_w$}  {:<7}  {:<7}  {:<7}  {:<7}  {:<5}  {:<35}  {}",
        "PROJECT",
        "SERVICE",
        "PID",
        "CPU",
        "MEM",
        "UPTIME",
        "PORT",
        "TUNNEL",
        "CWD",
        proj_w = proj_w,
        name_w = name_w
    );
    println!("{}", header);

    let mut distinct_projects = std::collections::HashSet::new();

    for r in rows {
        distinct_projects.insert(&r.project_key);
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
            "{:<proj_w$}  {:<name_w$}  {:<7}  {:<7}  {:<7}  {:<7}  {:<5}  {:<35}  {}",
            r.project_key,
            r.service_name,
            r.pid,
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

const PROCMAN_SKILL_CONTENT: &str = include_str!("../skills/procman/SKILL.md");

fn install_skill_file(custom_path: Option<&str>) -> Result<()> {
    let dest = if let Some(p) = custom_path {
        std::path::PathBuf::from(p)
    } else {
        std::path::PathBuf::from(".agents/skills/procman/SKILL.md")
    };

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&dest, PROCMAN_SKILL_CONTENT)?;
    println!(
        "✅ procman agent skill successfully installed to: {}",
        dest.display()
    );
    println!("   Your AI agent (Antigravity, Cursor, Claude Code) can now autonomously manage project processes.");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Trigger non-blocking background check if cache is older than 24 hours
    updater::trigger_background_update_check();

    let is_ui_command = matches!(cli.command, Some(Commands::Ui { .. }));
    let is_upgrade_command = matches!(cli.command, Some(Commands::Upgrade { .. }));
    let is_uninstall_command = matches!(cli.command, Some(Commands::Uninstall { .. }));

    match cli.command {
        Some(Commands::Start { name, force }) => {
            let (config_path, config) = require_config()?;
            let rows = process_manager::start(&config_path, &config, name.as_deref(), force)?;
            print_status(&rows);
        }
        Some(Commands::Stop { name }) => {
            let (config_path, config) = require_config()?;
            let rows = process_manager::stop(&config_path, &config, name.as_deref())?;
            print_status(&rows);
        }
        Some(Commands::Kill { name }) => {
            let (config_path, config) = require_config()?;
            let rows = process_manager::force_stop(&config_path, &config, name.as_deref())?;
            print_status(&rows);
        }
        Some(Commands::KillPort { port }) => {
            process_manager::kill_port(port);
            println!("Port {} force-freed.", port);
        }
        Some(Commands::Restart { name, force }) => {
            let (config_path, config) = require_config()?;
            let rows = process_manager::restart(&config_path, &config, name.as_deref(), force)?;
            print_status(&rows);
        }
        Some(Commands::Status { name }) => {
            let (config_path, config) = require_config()?;
            let rows = process_manager::status(&config_path, &config, name.as_deref())?;
            print_status(&rows);
        }
        Some(Commands::Ps) => {
            let global_rows = process_manager::scan_global_processes()?;
            print_global_ps(&global_rows);
        }
        Some(Commands::Logs {
            name,
            follow,
            lines,
        }) => {
            let (config_path, config) = require_config()?;
            let _ = config
                .processes
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!("Unknown process \"{}\"", name))?;

            let log_file = process_manager::log_file_for(&config_path, &name)?;
            print!("{}", logs::read_tail(&log_file, lines));

            if follow {
                logs::follow_log(&log_file, |chunk| {
                    print!("{}", chunk);
                });
            }
        }
        Some(Commands::Forward { name }) => {
            let (config_path, config) = require_config()?;
            let (url, _) = cloudflare::forward_start(&config_path, &config, &name)?;
            println!("{}", url);
            if let Some(qr_str) = qr::render_qr(&url) {
                println!("\n{}", qr_str);
            }
        }
        Some(Commands::ForwardStop { name }) => {
            let (config_path, _) = require_config()?;
            let stopped = cloudflare::forward_stop(&config_path, &name)?;
            if stopped {
                println!("tunnel stopped for {}", name);
            } else {
                println!("no tunnel running for {}", name);
            }
        }
        Some(Commands::Qr { name }) => {
            let (config_path, config) = require_config()?;
            let rows = process_manager::status(&config_path, &config, Some(&name))?;
            if let Some(row) = rows.into_iter().next() {
                if let Some(url) = row.tunnel_url {
                    println!("Tunnel URL: {}", url);
                    if let Some(qr_str) = qr::render_qr(&url) {
                        println!("\n{}", qr_str);
                    }
                } else if let Some(port) = row.port {
                    let local_url = format!("http://localhost:{}", port);
                    println!("Local URL: {}", local_url);
                    if let Some(qr_str) = qr::render_qr(&local_url) {
                        println!("\n{}", qr_str);
                    }
                } else {
                    println!("No active tunnel URL or port found for \"{}\"", name);
                }
            }
        }
        Some(Commands::Upgrade { yes }) => {
            updater::run_upgrade(yes)?;
        }
        Some(Commands::Uninstall { yes, purge }) => {
            uninstaller::run_uninstall(yes, purge)?;
        }
        Some(Commands::Skill { path }) => {
            install_skill_file(path.as_deref())?;
        }
        Some(Commands::Ui { all }) => {
            ui::render_ui(all)?;
        }
        None => {
            if let Some(config_path) = config::find_config_path(None) {
                let config = config::load_config(&config_path)?;
                let rows = process_manager::status(&config_path, &config, None)?;
                print_status(&rows);
            } else {
                println!("No procman config found in current directory tree.\n");
                println!("Active processes across your system (procman ps):");
                let global_rows = process_manager::scan_global_processes()?;
                print_global_ps(&global_rows);
            }
        }
    }

    if !is_ui_command && !is_upgrade_command && !is_uninstall_command {
        if let Some(notice) = updater::get_update_notice() {
            print!("{}", notice);
        }
    }

    Ok(())
}
