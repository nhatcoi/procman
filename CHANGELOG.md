# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Planned
- **Service Dependency Ordering (`depends_on`)**: Wait for upstream services to open ports or pass health checks before starting downstream dependencies.
- **File Watcher & Hot Reload (`watch`)**: Auto-restart processes on source code changes using the `notify` crate.
- **Environment File Loading (`env_file`)**: Automatically load `.env` or `.env.local` files globally or per process.
- **Auto-Recovery on Crash (`restart: on-failure`)**: Automatically restart failed processes with configurable retry limits (`max_retries`).
- **Pre-Start Tasks (`pre_start`)**: Execute prerequisite commands (e.g. database migrations, assets build) prior to launching the main process.
- **Project Initializer (`procman init`)**: Auto-detect project tech stack (Node.js, Go, Python, Rust, Docker) and generate a ready-to-use `procman.yaml`.
- **Diagnostic Assistant (`procman doctor <name>`)**: Analyze recent log lines to diagnose crash reasons and recommend fixes.
- **Named Tunnels & Multi-Provider Support**: Support persistent custom domains and additional tunnel providers (`ngrok`, `tailscale`).

---

## [0.1.0] - 2026-08-19

### Added
- **Daemonless Background Process Management**:
  - Spawns background processes in decoupled POSIX sessions (`setsid`) without needing a background daemon.
  - POSIX Process Group management (`-pid`) to ensure all child and sub-child processes are cleanly terminated.
  - Graceful shutdown lifecycle: sends `SIGTERM`, waits up to 5 seconds, and falls back to `SIGKILL` if unresponsive.
  - Non-intrusive liveness probe via POSIX Signal 0 (`kill(pid, 0)`).
- **Core CLI Commands**:
  - `procman start [name]` - Start all or specific processes in the background.
  - `procman stop [name]` - Gracefully stop all or specific processes.
  - `procman restart [name]` - Restart all or specific processes.
  - `procman kill [name]` - Force-kill (`SIGKILL`) and immediately free ports.
  - `procman kill-port <port>` - Forcibly terminate any process holding a given port.
  - `procman status` - Pretty-printed terminal status table.
  - `procman ps` / `procman ls` - List all active processes across all projects on the system.
  - `procman logs <name> [-f] [-n <lines>]` - View or live-stream (`tail -f`) logs.
  - `procman forward <name>` & `procman forward-stop <name>` - Cloudflare Quick Tunnel integration.
  - `procman qr <name>` - Print ASCII QR code in terminal for mobile scanning.
  - `procman skill` - Install procman AI agent skill into project (`.agents/skills/procman/SKILL.md`).
  - `procman upgrade` / `update` - Self-upgrade to the latest available release.
  - `procman uninstall` - Completely uninstall procman and stop background processes (`-p, --purge`).
  - `procman ui` - Interactive terminal TUI dashboard powered by `ratatui`.
- **Interactive TUI Dashboard**:
  - Realtime process status badges (`●` up / `○` down), CPU%, RAM (MB), PID, and Uptime.
  - Realtime streaming log view with keyword search & filtering (`/`).
  - Fullscreen log view (`Enter`/`Space`) with scrollback history navigation (`PageUp`/`PageDown`/`Home`/`End`).
  - Mobile QR code popup modal (`o`).
- **Cloudflare Quick Tunnels**:
  - Automatic public URL parsing (`https://*.trycloudflare.com`) via regex from log streams.
  - Automatic tunneling upon startup when `forward: true` is configured.
- **System Resource Monitoring**:
  - Realtime CPU% and Resident Memory (MB) tracking using `sysinfo`.
- **Isolated State & Log Storage (XDG Compliant)**:
  - SHA-1 project isolation under `~/.local/state/procman/<project_key>/state.json`.
  - Process logs written to `~/.local/share/procman/<project_key>/logs/<name>.log`.
- **Release Profile Optimizations**:
  - Aggressive binary optimizations in `Cargo.toml` (`opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`).
