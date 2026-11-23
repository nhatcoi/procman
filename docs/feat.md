# `procman` - Feature Roadmap & Tracking

A concise summary of the **implemented** features and the **planned** backlog for `procman`.

---

## 1. Implemented Features

### ⚙️ Core Process Manager
- **Daemonless**: Processes run in the background in their own session (`setsid`); no background daemon consumes resources.
- **POSIX process group management**: Cleans up the whole child process tree (`-pid`): `SIGTERM` -> 5s timeout -> `SIGKILL`.
- **Liveness probe**: Accurate alive/dead detection via POSIX Signal 0 (`kill(pid, 0)`).
- **Control commands**: `procman start`, `procman stop`, `procman restart`, `procman status`.

### 🛡️ Port Clearance & Force Kill
- **`free_port: true` (in YAML)**: Automatically kills whatever occupies the port before starting the server.
- **`procman kill [name]`**: Force-kills (`SIGKILL`) and frees the port.
- **`procman_kill_port` (MCP tool)**: Force-frees any stuck port for AI agents.
- **`-f, --forward` flag**: `procman start -f` and `procman restart -f` also open a Cloudflare tunnel.

### 🌐 Cloudflare Quick Tunnel (`forward: true`)
- **Automatic tunnel**: Set `forward: true` in YAML to run `cloudflared` in the background on start.
- **URL extraction**: Captures the `https://*.trycloudflare.com` link via regex and shows it in the status table.
- **CLI**: `procman forward <name>` and `procman unforward <name>`.

### 📜 Logs & State Isolation (XDG Standards)
- **Isolated state**: The project's absolute path is SHA-1 hashed and stored separately under `~/.local/state/procman/<project_key>/` and `~/.local/share/procman/<project_key>/logs/`.
- **View & stream logs**: `procman logs <name> -n 50` (last N lines) and `procman logs <name> -f` (live follow, like `tail -f`).

### 📊 Interactive Terminal Dashboard (`procman ui`)
- Built with **`ratatui` + `crossterm`** (immediate-mode rendering).
- Status table with colored dots (`●` up / `○` down), PID, uptime, port, and tunnel.
- Real-time scrolling log pane for the selected process.
- Key bindings: `↑`/`↓` select, `s` start, `x` stop, `k` force-kill, `r` restart, `f` forward, `u` unforward, `q` quit.

- **Terminal QR code**: the `o` key in `procman ui` shows a QR popup so a phone camera can open the URL directly.
- **Real-time CPU% & RAM (MB)**: CPU (%) and RAM (MB) per process in both `procman status` and `procman ui`.
- **Fullscreen log view (`Enter` / `Space`)**: Expands the log pane to 100% of the screen, with scrolling (`PageUp`/`PageDown`/`↑`/`↓`/`Home`/`End`).
- **Update notice & upgrade (`procman upgrade`)**: Checks GitHub Releases through a 24h background cache (no CLI slowdown), shows a banner, and can download/build the new version after a `[y/N]` confirmation.
- **Clean uninstall (`procman uninstall`)**: Warns about and stops running processes, removes the binary, and with `--purge` also deletes all state and log directories.
- **System-wide process scan (`procman ps` / `procman ls`)**: Scans every project and service running on the machine, showing project key, service, PID, CPU%, RAM, uptime, port, tunnel, and CWD. Triggers automatically when `procman` runs outside a repo.
- **Global TUI dashboard (`procman ui --all` / `Tab` key)**: View, live-tail logs, search, and control (stop, force-kill, QR) processes across all projects. `Tab` switches between the Local Project View and the Global System Dashboard.
- **Known Projects Registry**: Remembers every project that has been used in `~/.local/state/procman/registry.json`. Shows both running (`● up`) and stopped (`○ down`) services, and lets you press `s` to Start / `r` to Restart / `x` to Stop any service in any project from the Global Dashboard without `cd`. `d` / `Delete` removes a project from the registry.
- **Dependency ordering & readiness checks (`depends_on`)**: Declare dependencies (`depends_on: [db, redis]`); procman orders them with a topological sort, detects cycles, and probes the TCP port (`TcpStream::connect_timeout`) before starting downstream services.
- **Auto-restart on code change (`watch` / `procman watch`)**: Real-time file watching via the `notify` crate; restarts the process with a 350ms debounce and ignores noisy directories (`.git`, `target`, `node_modules`, `logs`). Configure in YAML (`watch: true` / `watch: ["src"]`, `watch_ignore: ["temp"]`) or via `procman watch [name]`.
- **AI agent skill installer (`procman skill`)**: Instantly installs the skill file (`.agents/skills/procman/SKILL.md`) into any project using static embedding (`include_str!`), so AI assistants (Antigravity, Cursor, Claude Code, Codex) can understand and manage the project.
- **Diagnosis & self-repair (`procman doctor`)**: Two-tier diagnostics. Tier 1 (internal rule engine) analyzes common failures in under 1ms, fully offline: port conflicts, missing modules/packages, missing executables, PostgreSQL/MySQL/Redis connection errors, missing database migrations, Linux OOM killer (exit 137), permission denied, and missing env vars. Tier 2 (AI agent delegation) detects and hands the log + config context to an AI agent CLI (`agy`, `claude`, `codex`, `gemini`, `ollama`) for root-cause analysis (RCA), and supports `-f, --fix` to run the suggested fix automatically.

---

## 2. Planned Features / Backlog

### 🎨 TUI Experience & Global Management
- [x] 📱 **Terminal QR code**: Show a QR code for the Cloudflare tunnel URL so a phone can open the web app for testing immediately.
- [x] 📊 **CPU% & RAM (MB)**: Two real-time resource columns per process in the status table.
- [x] 🔍 **Log search & filter (`/`)**: Type a keyword to filter log lines live (`error`, `500`, `panic`).
- [x] 🪟 **Fullscreen log view (`Enter`)**: Expand the log pane to scroll through history (`PageUp`/`PageDown`).
- [x] 🌐 **System-wide process scan (`procman ps`)**: List every background service on the machine, like `docker ps` / `pm2 list`.
- [x] 🔄 **Update notice & upgrade (`procman upgrade`)**: Detect new releases and upgrade quickly.
- [x] 🗑️ **Automatic uninstall (`procman uninstall`)**: Clean up processes, remove the binary, and optionally purge all state/logs.

### ⛓️ Orchestration & Reliability
- [x] 🔗 **Dependency ordering (`depends_on`)**: Resolve the dependency tree (topological sort + cycle detection) and wait for a dependency's port to open or become ready before starting the next service.
- [x] 🔄 **Auto-reload on code change (`watch` / `procman watch`)**: Restart processes (Go, Python, Rust, C++) when files change, using `notify` with a 350ms debounce and automatic filtering of noisy paths (`.git`, `target`, `node_modules`, `logs`).
- [ ] 📄 **Environment files (`env_file`)**: Load `.env` or `.env.local` per service or for the whole project.
- [ ] 🛡️ **Restart on crash (`restart: on-failure`)**: Recover a process that exits with a non-zero code (`max_retries: 3`).
- [ ] 🚀 **Pre-start tasks (`pre_start`)**: Run preparation commands (database migrations, builds) before spawning the main server.

### 🤖 Smart Tooling & AI
- [x] 🩺 **Diagnosis & self-repair (`procman doctor [name] [-s] [--ai] [-f]`)**: Offers two modes by default (`[1] Spot Check` - instant rule-engine scan, `[2] AI Check` - full AI analysis with a status table of all processes, running or stopped). Automatically extracts logs, exit codes, and port/RAM data so the AI can produce a concise report and an exact fix command (`--fix` runs the fix automatically).
- [x] 🪄 **Project scan & config generation (`procman init [dir] [--ai] [-y]`)**: Analyzes the source tree (reads `package.json`, `Cargo.toml`, `go.mod`, `docker-compose.yml`, `Makefile`, `pyproject.toml`, `requirements.txt`, `.env.example`), detects frameworks, commands, ports, and dependencies, and generates a proper `procman.yaml` (instant heuristic scanner combined with an AI agent CLI generator).
- [x] 🔌 **AI agent integration (`procman mcp [-d, --dir <dir>]`)**: A Model Context Protocol (MCP) server over JSON-RPC 2.0 stdio transport, letting AI assistants (Antigravity, Cursor, Claude Code, Codex, Windsurf) list services (`procman_status`), control the lifecycle (`procman_start`, `procman_stop`, `procman_restart`), read live logs (`procman_logs`), run diagnostics (`procman_doctor`), free ports (`procman_kill_port`), and generate configs (`procman_init`), enabling a closed fix-and-test loop.
- [ ] 💬 **In-dashboard AI assistant (`procman ui` - `?` / `a` key)**: A modal AI chat popup inside the TUI for quick questions ("Why is `api` using so much RAM?", "Summarize the latest 500 errors") based on live in-memory logs and metrics.
- [ ] 🌐 **Custom domains & multiple providers**: Support Named Tunnels (fixed custom domain) and other providers (`ngrok`, `tailscale`).
