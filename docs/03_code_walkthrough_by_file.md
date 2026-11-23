# 03. Code Walkthrough by File

The structure, purpose, and Rust techniques used in each part of `procman`.

---

## 1. [`src/main.rs`](../src/main.rs) & [`src/cli/`](../src/cli) - CLI Entry Point & Routing

* **`#[derive(Parser)]` & `#[derive(Subcommand)]`**: `clap` uses procedural macros to turn structs and enums into a full command-line parser with help output and type checking, without boilerplate.
* **Subcommands**: defined in `src/cli/args.rs`; each has a handler in `src/cli/commands/`.
* **`match cli.command`**: Rust's exhaustive pattern matching guarantees every subcommand is handled.
* **Status table**: `src/cli/format.rs` computes column widths dynamically so the table stays aligned.

---

## 2. [`src/engine/config.rs`](../src/engine/config.rs) - Config Discovery & Loading

```rust
pub fn find_config_path(start_dir: Option<&Path>) -> Option<PathBuf>
```

* **Upward directory traversal**:
  * Starts at the current directory (`env::current_dir()`) and looks for candidate files (`procman.yaml`, `procman.json`, `.procman.yaml`, ...).
  * If none is found, `dir.parent()` moves up one level until it reaches `$HOME` or `/`.
* **YAML & JSON support**: a `.json` extension is parsed with `serde_json::from_str`, anything else with `serde_yaml::from_str`.
* **`Option<T>` instead of `null`**: returns `None` when no config exists.

---

## 3. [`src/engine/paths.rs`](../src/engine/paths.rs) - XDG Paths & Project Isolation

```rust
pub fn project_key(config_path: &Path) -> Result<String>
```

* **XDG Base Directory**:
  * State (`state.json`) lives in `$XDG_STATE_HOME` or `~/.local/state/procman/<project_key>/`.
  * Logs (`<name>.log`) live in `$XDG_DATA_HOME` or `~/.local/share/procman/<project_key>/logs/`.
* **Unique `project_key`**:
  * SHA-1 of the canonical absolute config path (`fs::canonicalize`).
  * The parent directory name plus the first 8 hash characters (e.g. `chat-chit-8a2f1b0c`), so same-named projects in different directories never overwrite each other's state or logs.

---

## 4. [`src/engine/state.rs`](../src/engine/state.rs) - State Persistence & Liveness Probe

```rust
pub fn is_alive(pid: i32) -> bool
```

* **POSIX Signal 0**: `nix::sys::signal::kill(Pid::from_raw(pid), None)` checks whether a process exists without sending a real signal.
* **Structured state file**: process entries (`ProcEntry`) and tunnel entries (`TunnelEntry`) are stored as JSON.

---

## 5. [`src/engine/supervisor.rs`](../src/engine/supervisor.rs) - Process Lifecycle

```rust
pub fn start_one(config_path: &Path, name: &str, def: &ProcessDef, state: &mut State) -> Result<ProcEntry>
```

* **Detached background spawn**:
  * Opens the log file in append mode: `OpenOptions::new().create(true).append(true).open(&log_file)`.
  * Redirects `stdout` and `stderr` straight to the log file so the terminal buffer never blocks.
  * Creates a new session with `setsid()` inside `cmd.pre_exec`:
    ```rust
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }
    ```
* **Clean process-tree shutdown**:
  * Sends `SIGTERM` to `-pid` (a negative PID targets the whole process group).
  * Waits up to the configured timeout; if the process is still alive, sends `SIGKILL`.
* **Dependencies**: orders services with a topological sort, detects cycles, and probes TCP ports before starting downstream services.

---

## 6. [`src/tunnels/cloudflare.rs`](../src/tunnels/cloudflare.rs) & [`qr.rs`](../src/tunnels/qr.rs) - Cloudflare Tunnel & QR

```rust
pub fn forward_start(config_path: &Path, config: &Config, name: &str) -> Result<(String, i32)>
```

* **`cloudflared` check**: runs `cloudflared --version` first to report a clear error if it is missing.
* **Quick Tunnel**: runs `cloudflared tunnel --url http://localhost:<port>` in the background.
* **URL extraction**: tails the tunnel log and matches `r"https://[a-zA-Z0-9-]+\.trycloudflare\.com"` to capture the public URL.
* **QR code**: `qr.rs` renders the URL as a terminal QR code (shown in the TUI with `o`).

---

## 7. [`src/tui/`](../src/tui) - Interactive Terminal UI (Ratatui)

* `mod.rs` - terminal setup and the main event loop.
* `app.rs` - application state (local/global mode, selection, pending actions).
* `events.rs` - key handling.
* `actions.rs` - background actions (start/stop/kill/forward).
* `render.rs` - drawing.
* `logview.rs` - bounded, incremental log buffer with filtering.

Key points:
* **Terminal setup**: `enable_raw_mode()?`, `execute!(stdout, EnterAlternateScreen)?`; both are restored on exit (`LeaveAlternateScreen`, `disable_raw_mode`).
* **Rendering**: `ratatui::widgets::Table` for the process list (green `●` running, red `○` stopped) and `Paragraph` for the scrolling log pane.
* **Non-blocking input**: `event::poll(Duration::from_millis(150))` keeps the UI responsive to `↑`/`↓`, `s`, `x`, `r`, `f`, `u`, `q`, and the other bindings.

---

## 8. [`src/cli/commands/updater.rs`](../src/cli/commands/updater.rs) & [`src/engine/telemetry.rs`](../src/engine/telemetry.rs) - Update Check & Self-Upgrade

```rust
pub fn check_latest_version(force: bool) -> Option<String>
pub fn run_upgrade(yes: bool) -> Result<()>
```

* **24h cache, non-blocking check**:
  * The cache lives at `~/.local/state/procman/update_cache.json`.
  * During ordinary commands, a background thread refreshes the version when the cache is older than 24h, without delaying the CLI.
* **Real SemVer comparison**: versions are split into `(major, minor, patch)` tuples and compared numerically, not as strings.
* **`[y/N]` confirmation & install**:
  * `procman upgrade` shows current vs. latest version and asks for confirmation (`--tag` targets a specific release).
  * It prefers `cargo install --git ... --force`, or runs `install.sh`.

---

## 9. [`src/cli/commands/skill.rs`](../src/cli/commands/skill.rs) - AI Agent Skill Setup

```rust
pub const EMBEDDED_SKILL_MD: &str = include_str!("../../../skills/procman/SKILL.md");
pub fn execute(dir: &str, force: bool) -> Result<()>
```

* **Skill embedded in the binary (`include_str!`)**: `SKILL.md` is compiled into the `procman` binary, so `procman skill` installs instantly in any project, offline, with no external script.
* **Directory handling & update check**:
  * Accepts a directory or a `.md` path and creates parent directories (`.agents/skills/procman`).
  * Compares existing content and reports when the skill is already current (`✨ procman skill is already up-to-date`).

---

## 10. [`src/engine/doctor/`](../src/engine/doctor/mod.rs) & [`src/cli/commands/doctor.rs`](../src/cli/commands/doctor.rs) - Diagnostic Engine & AI Agent Delegation

```rust
pub fn diagnose_all(config_path: &Path, config: &Config, target_name: Option<&str>, force_ai: bool, agent_override: Option<&str>) -> Result<Vec<DiagnosticReport>>
```

* **Two-tier architecture**:
  * **Tier 1 (local rule & signature matcher)**: `rules.rs` and `database.rs` hold 24+ predefined signatures for common failures: port conflicts (`EADDRINUSE`), missing npm/pip/go packages, missing system binaries, Postgres/MySQL/Redis connection failures, missing migrations, Linux OOM killer (exit 137), permission denied, missing env vars, and runtime panics. Runs in under 1ms, fully offline, at zero token cost.
  * **Tier 2 (AI agent CLI delegation)**: `agent.rs` detects installed AI agent CLIs (`agy`, `claude`, `codex`, `gemini`, `ollama`), bundles context (last 50 log lines + process metadata + `procman.yaml`), and hands it over for root-cause analysis (RCA).
* **Automatic fixes (`--fix`)**: `doctor.rs` collects remediation commands from the rule engine or the AI agent, then asks `[y/N]` or runs them directly with `--fix`.

---

## 11. [`src/engine/init/`](../src/engine/init/mod.rs) & [`src/cli/commands/init.rs`](../src/cli/commands/init.rs) - Project Scanner & Config Generator

```rust
pub fn generate_initial_config(root_dir: &Path, force_ai: bool, agent_override: Option<&str>) -> Result<(String, String)>
```

* **Heuristic scanner (`scanner.rs`)**:
  * Reads manifests (`package.json`, `Cargo.toml`, `go.mod`, `docker-compose.yml`, `pyproject.toml`, `requirements.txt`, `Makefile`, `.env`).
  * Detects monorepo/multi-service layouts (`apps/*`, `packages/*`, `frontend`, `backend`, `api`, `web`).
  * Infers default ports (3000, 5173, 8000, 8080, 5432, 6379) and start commands (`npm run dev`, `cargo run`, `uvicorn`, `go run .`).
* **AI generation (`ai.rs`)**:
  * Sends the directory tree and manifest contents to an AI agent CLI (`claude`, `gemini`, `agy`, `codex`, `ollama`) to design `procman.yaml`, including `depends_on`.
  * Validates the YAML before showing it.
* **Safe preview & write (`init.rs`)**: shows the generated config and asks for confirmation before writing (`-y` to skip, `-f` to overwrite).

---

## 12. [`src/engine/mcp/`](../src/engine/mcp/mod.rs) & [`src/cli/commands/mcp.rs`](../src/cli/commands/mcp.rs) - Model Context Protocol (MCP) Server

```rust
pub fn run_stdio_server(project_dir: Option<&Path>) -> Result<()>
```

* **JSON-RPC 2.0 (`protocol.rs` & `server.rs`)**:
  * Two-way communication with AI agents (Antigravity, Cursor, Claude Code, Codex, Windsurf) over `stdin`/`stdout`.
  * Handles the full MCP handshake and methods: `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/read`.
* **Tools (9)**:
  * `procman_status`: status table with PID, port, CPU%, RAM (MB), and uptime in real time.
  * `procman_start`, `procman_stop`, `procman_restart`: background process lifecycle (supports `force`).
  * `procman_logs`: the last `N` log lines of a process.
  * `procman_doctor`: crash diagnosis (rule engine) or process health review (AI check).
  * `procman_kill_port`: force-kills whatever occupies a port.
  * `procman_ps`: lists every procman process on the machine.
  * `procman_init`: analyzes the codebase and generates `procman.yaml`.
* **Resources**:
  * `procman://processes`: a JSON snapshot of process state.
  * `procman://logs/{name}`: log contents as text.
