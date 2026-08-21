use clap::{Parser, Subcommand};

pub const APP_NAME: &str = "procman";
pub const APP_AUTHOR: &str = "nhatcoi";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_ABOUT: &str =
    "Custom process manager: start/stop/log/forward processes from a project config file";

#[derive(Parser, Debug)]
#[command(
    name = APP_NAME,
    author = APP_AUTHOR,
    version = APP_VERSION,
    about = APP_ABOUT
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start all processes or a specific process
    Start {
        /// Optional name of a specific process to start
        name: Option<String>,
        /// Automatically start a Cloudflare tunnel for the process (requires port in procman.yaml)
        #[arg(short, long)]
        forward: bool,
    },
    /// Stop all processes or a specific process
    Stop {
        /// Optional name of a specific process to stop
        name: Option<String>,
    },
    /// Force kill all processes or a specific process (SIGKILL) and free occupied ports
    Kill {
        /// Optional name of a specific process to force kill
        name: Option<String>,
    },
    /// Restart all processes or a specific process
    Restart {
        /// Optional name of a specific process to restart
        name: Option<String>,
        /// Automatically start a Cloudflare tunnel for the process
        #[arg(short, long)]
        forward: bool,
    },
    /// Show running status and metrics (default if no command given)
    Status {
        /// Optional name of a specific process to inspect
        name: Option<String>,
    },
    /// View process logs (supports --follow)
    Logs {
        /// Process name whose logs to view
        name: String,
        /// Follow/tail log output continuously
        #[arg(short, long)]
        follow: bool,
        /// Number of recent lines to display
        #[arg(short = 'n', long, default_value_t = 100)]
        lines: usize,
    },
    /// Watch files and auto-restart processes on source code changes
    Watch {
        /// Optional name of a specific process to watch (watches all configured processes if omitted)
        name: Option<String>,
    },
    /// Start a Cloudflare Quick Tunnel for a service
    Forward {
        /// Process name to forward (must have a port defined)
        name: String,
    },
    /// Stop the Cloudflare Quick Tunnel for a service
    Unforward {
        /// Process name whose tunnel to stop
        name: String,
    },
    /// Interactive Terminal UI dashboard (TUI)
    Ui {
        /// Open directly in Global Dashboard showing all projects on the system
        #[arg(short = 'a', short_alias = 'g', long = "all", alias = "global")]
        all: bool,
    },
    /// Scan and inspect all running processes across all projects on the machine
    #[command(alias = "ls")]
    Ps,
    /// Upgrade procman to the latest released version from GitHub
    Upgrade {
        /// Target a specific release tag/version (e.g. v0.1.0) instead of latest
        #[arg(long)]
        tag: Option<String>,
    },
    /// Uninstall procman from the system, stop all running processes and optionally purge state/logs
    Uninstall {
        /// Skip interactive confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
        /// Also purge all state and log files (~/.local/state/procman, ~/.local/share/procman)
        #[arg(long)]
        purge: bool,
    },
    /// Install procman AI agent skill into project (.agents/skills/procman/SKILL.md)
    Skill {
        /// Target directory or file path to install SKILL.md into
        #[arg(default_value = ".agents/skills/procman")]
        dir: String,
        /// Force overwrite existing skill file even if content matches
        #[arg(short, long)]
        force: bool,
    },
    /// Diagnose processes, analyze root causes, and suggest/apply fixes (Spot Check & AI Check)
    Doctor {
        /// Optional name of a specific process to diagnose (diagnoses all processes if omitted)
        name: Option<String>,
        /// Run fast Spot Check (Instant Rule Engine & Status Scanner) without prompt
        #[arg(short, long)]
        spot: bool,
        /// Run deep AI Check using local AI Agent CLI (agy, claude, codex, gemini, ollama)
        #[arg(long)]
        ai: bool,
        /// Explicitly choose the AI agent CLI executable to invoke
        #[arg(long)]
        agent: Option<String>,
        /// Automatically execute the suggested remediation command
        #[arg(short, long)]
        fix: bool,
    },
    /// Automatically scan project and generate procman.yaml configuration
    Init {
        /// Target directory to scan (defaults to current directory)
        #[arg(default_value = ".")]
        dir: String,
        /// Force deep analysis and generation using local AI Agent CLI (agy, claude, codex, gemini, ollama)
        #[arg(long)]
        ai: bool,
        /// Explicitly choose the AI agent CLI executable to invoke
        #[arg(long)]
        agent: Option<String>,
        /// Skip interactive confirmation and write procman.yaml directly
        #[arg(short, long)]
        yes: bool,
        /// Overwrite existing procman.yaml if present
        #[arg(short, long)]
        force: bool,
    },
    /// Run Model Context Protocol (MCP) server for AI assistants (Antigravity, Claude, Cursor, Codex)
    Mcp {
        /// Optional project root directory (defaults to current directory)
        #[arg(short, long)]
        dir: Option<String>,
    },
}


