# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Dual-Mode Diagnostic Assistant & Auto-Fixer (`procman doctor [name] [-s] [--ai] [-f]`)**:
  - **Interactive 2-Option Selector**: Default menu prompting between `[1] Spot Check` (fast rule scanner) and `[2] AI Check` (deep AI process analysis & health assessment).
  - **Spot Check Mode (`-s, --spot`)**: Instant offline rule engine (< 1ms) matching 24+ signature patterns (ports, missing packages, exit codes, DB connection issues, OOM 137).
  - **AI Check Mode (`--ai`)**: System-wide process health snapshot analyzing logs and runtime status (even for active/running services) and returning a concise, visual Markdown table, key findings, and actionable fix commands using local AI agent CLIs (`claude`, `gemini`, `agy`, `codex`, `ollama`).
  - **Automated Remediation (`--fix` / `-f`)**: Interactively prompts or automatically executes the suggested fix commands.
- **Automatic Project Scanner & Config Generator (`procman init [dir] [--ai] [-y]`)**:
  - **Heuristic Scanner**: Automatically inspects codebase structure and manifest files (`package.json`, `Cargo.toml`, `go.mod`, `docker-compose.yml`, `pyproject.toml`, `requirements.txt`, `Makefile`, `.env`) to detect frontend, backend, workers, and database dependencies.
  - **Monorepo / Multi-Service Support**: Automatically detects subprojects in `apps/*`, `packages/*`, `frontend/`, `backend/`, `api/`, setting proper `cwd` and port conventions.
  - **AI-Powered Config Generation (`--ai`)**: Generates optimized, production-grade `procman.yaml` configurations using local AI coding assistants (`claude`, `gemini`, `agy`, `codex`, `ollama`) with automatic YAML syntax validation.
  - **Interactive Preview & Safe Overwrite**: Displays clean configuration preview and confirms before writing (`-y` to skip, `-f` to overwrite).
- **Model Context Protocol (MCP) Server (`procman mcp [-d <dir>]`)**:
  - **Standardized JSON-RPC 2.0 stdio Transport**: Connects autonomous AI coding assistants (Antigravity, Cursor, Claude Code, Codex, Windsurf) to local procman instances.
  - **Comprehensive Tool Suite (9 Tools)**: `procman_status`, `procman_start`, `procman_stop`, `procman_restart`, `procman_logs`, `procman_doctor`, `procman_kill_port`, `procman_ps`, `procman_init`.
  - **Dynamic Resources**: Exposes `procman://processes` (structured process status) and `procman://logs/{name}` (live log stream) for agent context injection.
- **AI Agent Skill Subcommand (`procman skill`)**:
  - Native CLI subcommand to install the procman AI agent skill (`.agents/skills/procman/SKILL.md`) into any project.
  - Fully self-contained via compile-time `include_str!` embedding without requiring external shell scripts or internet connectivity.
  - Automatically handles destination directory resolution, parent folder creation, and up-to-date checks.

### Fixed
- **Global Dashboard Automatic Project Discovery & Auto-Healing**:
  - Automatically recovers and registers un-registered or historical projects from state directories (`~/.local/state/procman/*`) by resolving their configuration path from `state.json` or process working directories (`cwd`).
  - Ensures stopped services from all known/historical projects are always visible in the Global Dashboard (`procman ui --all` / `procman ps`) and can be managed directly.
  - Persists `config_path` in `state.json` on state save and auto-registers local projects when starting TUI or running CLI commands.

### Planned
- **Environment File Loading (`env_file`)**: Automatically load `.env` or `.env.local` files globally or per process.
- **Auto-Recovery on Crash (`restart: on-failure`)**: Automatically restart failed processes with configurable retry limits (`max_retries`).
- **Pre-Start Tasks (`pre_start`)**: Execute prerequisite commands (e.g. database migrations, assets build) prior to launching the main process.
- **Zero-Config AI Initializer (`procman init`)**: Auto-detect project tech stack and generate complete, dependency-aware `procman.yaml`.
- **Model Context Protocol Server (`procman mcp`)**: Standard MCP interface enabling AI coding assistants to manage background processes natively.
- **TUI Embedded AI Assistant (`procman ui` Ask Modal)**: In-dashboard AI assistant to query runtime metrics and inspect crash logs interactively.




---

## [0.1.3] - 2026-08-20

### Added
- **Service Dependency Ordering & Port Probing (`depends_on`)**:
  - Declarative dependency management in `procman.yaml` (`depends_on: [db, redis]`).
  - Automatic Topological Sorting and cycle detection (`A -> B -> A` fails fast with clear diagnostic path).
  - TCP port readiness probing via `std::net::TcpStream::connect_timeout` ensuring upstream dependencies open their listening ports before downstream services are spawned.
  - Starting a specific service (e.g. `procman start web`) automatically starts and waits for any unstarted upstream dependencies first.
- **Live File Watcher & Hot Reload (`watch` / `procman watch`)**:
  - Cross-platform file watcher powered by the `notify` crate.
  - Automatic process restart upon source code change with a 350ms debounce window.
  - Built-in noise and artifact filtering (`.git`, `target`, `node_modules`, `logs`, `.procman`, `.tmp`).
  - Declarative configuration in `procman.yaml` (`watch: true` or `watch: ["src"]`, `watch_ignore: ["temp"]`) and CLI command `procman watch [name]`.
- **Cross-Platform OS Support (Linux, macOS, Windows)**:
  - Full native support for Windows (`x86_64-pc-windows-msvc`), macOS (Intel & Apple Silicon), and Linux (x86_64 & ARM64).
  - Cross-platform process detachment, tree termination (`taskkill` on Windows, POSIX process groups on Unix), and port clearance.
  - Dedicated PowerShell installer `install.ps1` for Windows users.
- **In-App First-Run Telemetry (`engine::telemetry`)**:
  - Embedded non-blocking anonymous first-run ping executed once per unique installation.
  - Opt-out support via `DO_NOT_TRACK=1` and `PROCMAN_NO_TELEMETRY=1`.
- **Repository Views & Release Downloads Tracking**:
  - Read-only GitHub Release downloads badge and repo views counter in `README.md`.

---

## [0.1.2] - 2026-08-19

### Added
- **Global TUI Dashboard (`procman ui --all` / `-a` & `Tab` Key Switcher)**:
  - Interactive TUI dashboard capable of monitoring and controlling active processes across all projects on the entire system.
  - Seamless `Tab` key shortcut to switch back and forth between Local Project View and Global System Dashboard, or drill down into any selected project directly from the Global View.
  - Live log streaming, fullscreen view, search/filtering, QR popup, and stop/kill controls directly on global processes.
  - Automatic fallback to Global TUI Dashboard when launching `procman ui` outside any repository.
- **Known Projects Registry & Remote Control (`~/.local/state/procman/registry.json`)**:
  - Automatically records and persists all activated projects on the machine.
  - Global Dashboard displays both running (`● up`) and stopped (`○ down`) services across all known projects.
  - Remote service lifecycle control directly from Global View: Start (`s`), Restart (`r`), Stop (`x`), Force Kill (`k`), Forward (`f`), and Unregister (`d`).

### Changed
- **Clean Architecture & Modular Layering**:
  - Refactored `src/` into structured domain layers: `cli/`, `engine/` (`process`, `metrics`, `registry`, `state`, `logs`), `tunnels/`, and `tui/`.
- **Async TUI Execution & Non-Blocking State**:
  - Eliminated TUI rendering freezes during process lifecycle commands and Cloudflare tunnel spawning by offloading operations to background threads.
  - Introduced transient intermediate states (`starting...`, `stopping...`) in the TUI for immediate visual feedback.
- **Accurate Resource Aggregation & Deterministic UI**:
  - Aggregated CPU% and RAM metrics across full recursive child process trees while filtering out OS threads to prevent duplicate memory accounting.
  - Preserved row selection identity and deterministic sorting across state ticks.
- **Git Tag Installation**:
  - Updated installer (`install.sh`) and self-updater to install the latest released git tag by default.

---

## [0.1.1] - 2026-08-19

### Added
- **Global Process Scanning (`procman ps` / `procman ls`)**:
  - Scan and inspect all running processes across all projects on the entire system with PIDs, CPU%, RAM, Uptime, Ports, Tunnels, and CWD.
  - Intelligent fallback: running `procman` outside any repository automatically lists active processes across the entire machine instead of failing with an error.
- **Self-Uninstall Command (`procman uninstall` / `self-uninstall`)**:
  - Automatic detection and graceful termination of running processes across the machine before uninstallation.
  - Safe interactive prompt with `-y, --yes` flag to bypass.
  - Optional `-p, --purge` flag to completely wipe state (`~/.local/state/procman`) and logs (`~/.local/share/procman`) directories.

---

## [0.1.0] - 2026-08-19

### Added
- **Daemonless Background Process Management**:
  - Spawns background processes in decoupled POSIX sessions (`setsid`) without needing a background daemon.
  - POSIX Process Group management (`-pid`) to ensure all child and sub-child processes are cleanly terminated.
  - Graceful shutdown lifecycle: sends `SIGTERM`, waits up to 5 seconds, and falls back to `SIGKILL` if unresponsive.
  - Non-intrusive liveness probe via POSIX Signal 0 (`kill(pid, 0)`).
- **Core CLI Commands**:
  - `procman start [name]` - Start all or specific processes in the background (`-f, --force` to free ports).
  - `procman stop [name]` - Gracefully stop all or specific processes.
  - `procman restart [name]` - Restart all or specific processes.
  - `procman kill [name]` - Force-kill (`SIGKILL`) and immediately free ports.
  - `procman kill-port <port>` - Forcibly terminate any process holding a given port.
  - `procman status` - Pretty-printed terminal status table.
  - `procman logs <name> [-f] [-n <lines>]` - View or live-stream (`tail -f`) logs.
  - `procman forward <name>` & `procman forward-stop <name>` - Cloudflare Quick Tunnel integration.
  - `procman qr <name>` - Print ASCII QR code in terminal for mobile scanning.
  - `procman skill` - Install procman AI agent skill into project (`.agents/skills/procman/SKILL.md`).
  - `procman upgrade` / `update` - Self-upgrade to the latest available release.
  - `procman ui` - Interactive terminal TUI dashboard powered by `ratatui` + `crossterm`.
- **Interactive TUI Dashboard**:
  - Realtime process status badges (`●` up / `○` down), CPU%, RAM (MB), PID, and Uptime.
  - Realtime streaming log view with keyword search & filtering (`/`).
  - Fullscreen log view (`Enter`/`Space`) with scrollback history navigation (`PageUp`/`PageDown`/`Home`/`End`).
  - Mobile QR code popup modal (`o`).
- **Cloudflare Quick Tunnels**:
  - Automatic public URL parsing (`https://*.trycloudflare.com`) via regex from log streams.
  - Automatic tunneling upon startup when `forward: true` is configured in `procman.yaml`.
- **System Resource Monitoring**:
  - Realtime CPU% and Resident Memory (MB) tracking using `sysinfo`.
- **Isolated State & Log Storage (XDG Compliant)**:
  - SHA-1 project isolation under `~/.local/state/procman/<project_key>/state.json`.
  - Process logs written to `~/.local/share/procman/<project_key>/logs/<name>.log`.
- **Release Profile Optimizations**:
  - Aggressive binary optimizations in `Cargo.toml` (`opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`).
