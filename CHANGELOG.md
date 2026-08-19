# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Global TUI Dashboard (`procman ui --all` / `-a` & `Tab` Key Switcher)**:
  - Interactive TUI dashboard capable of monitoring and controlling active processes across all projects on the entire system.
  - Seamless `Tab` key shortcut to switch back and forth between Local Project View and Global System Dashboard.
  - Live log streaming, fullscreen view, search/filtering, QR popup, and stop/kill controls directly on global processes.
  - Automatic fallback to Global TUI Dashboard when launching `procman ui` outside any repository.

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
