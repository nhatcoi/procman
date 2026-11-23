# procman

[![Views](https://hits.sh/github.com/nhatcoi/procman.svg?label=views&color=007ec6)](https://hits.sh/github.com/nhatcoi/procman/)
[![Downloads](https://img.shields.io/github/downloads/nhatcoi/procman/total?label=downloads&color=4c1)](https://github.com/nhatcoi/procman/releases)
[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-Support-orange?style=flat&logo=buy-me-a-coffee)](https://buymeacoffee.com/nhatcoi)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> A fast, daemonless background process manager for local development, written in Rust.

Define your dev stack (APIs, frontends, workers) in one `procman.yaml`, then start, watch, tunnel, and debug it from the CLI or an interactive TUI. Includes dependency ordering, CPU/RAM metrics, Cloudflare quick tunnels, and an MCP server for AI agents.

---

## Install

**Linux & macOS** (pre-built binary, no Rust needed):
```bash
curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/install.sh | sh
# Specific version:
VERSION=v0.1.3 curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/nhatcoi/procman/main/install.ps1 | iex
```

**Cargo:**
```bash
cargo install --git https://github.com/nhatcoi/procman.git --tag v0.1.3 --force   # release
cargo install --git https://github.com/nhatcoi/procman.git --force                # main branch
```

**From source:**
```bash
git clone https://github.com/nhatcoi/procman.git && cd procman
cargo install --path . --force
```

Make sure `~/.cargo/bin` is on your `PATH`, then check with `procman --version`.

---

## Quick Start

**1. Create a config** (detects frameworks, ports, and dependencies):
```bash
procman init          # heuristic scan
procman init --ai     # generate with a local AI agent CLI
```

Example `procman.yaml`:
```yaml
processes:
  server:
    cmd: "npm run dev:server"
    port: 3001
    free_port: true        # kill whatever holds port 3001 before starting
    env:
      NODE_ENV: "development"

  web:
    cmd: "npm run dev"
    cwd: "./web"
    port: 5173
    depends_on: [server]   # waits for port 3001 before starting
    forward: true          # opens a Cloudflare quick tunnel

  worker:
    cmd: "python -m worker.main"
    cwd: "./services/worker"
    depends_on: [server]
```

**2. Run it:**
```bash
procman start             # all processes (or: procman start server)
procman status            # status, CPU, RAM, uptime, ports, tunnels
procman logs server -f    # follow logs
procman ui                # interactive dashboard
procman watch             # auto-restart on file changes
procman doctor            # diagnose crashes
```

```text
NAME    STATUS   PID      CPU      MEM      UPTIME   PORT   TUNNEL
server  up       124581   0.8%     38MB     14m20s   3001   -
web     up       124589   0.2%     54MB     14m18s   5173   https://sample-domain.trycloudflare.com
worker  up       124602   0.0%     22MB     14m15s   -      -
```

---

## Configuration

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `cmd` | string | **Yes** | Shell command to run (e.g. `"npm run dev"`) |
| `cwd` | string | No | Working directory relative to the config (default `"."`) |
| `port` | number | No | Listening port (needed for tunnels and readiness checks) |
| `free_port` | boolean | No | Kill any process holding `port` before starting |
| `forward` | boolean | No | Start a Cloudflare quick tunnel on start (default `false`) |
| `depends_on` | list | No | Services that must be ready (port open) first |
| `watch` | boolean or list | No | Auto-restart on changes: `true` or a list of paths |
| `watch_ignore` | list | No | Extra paths to ignore when watching |
| `log_file` | string | No | Custom log file name |
| `env` | map | No | Environment variables |

The config is `procman.yaml` or `procman.json`, found by searching upward from the current directory.

---

## CLI Reference

| Command | Description |
| :--- | :--- |
| `procman` / `procman status [name]` | Status of the current project (global list outside a repo) |
| `procman init [dir] [--ai] [-y] [-f]` | Scan the codebase and generate `procman.yaml` |
| `procman ps` / `procman ls` | List running processes across all projects |
| `procman start [name] [-f]` | Start all or one process (`-f, --forward` also opens a tunnel) |
| `procman stop [name]` | Graceful stop (`SIGTERM`, then `SIGKILL`) |
| `procman restart [name] [-f]` | Restart all or one process (`-f, --forward` opens a tunnel) |
| `procman kill [name]` | Force kill (`SIGKILL`) and free ports |
| `procman logs <name> [-f] [-n N]` | Show or follow logs |
| `procman watch [name]` | Restart on source changes (350ms debounce) |
| `procman forward <name>` | Open a Cloudflare quick tunnel (needs `cloudflared`) |
| `procman unforward <name>` | Close the tunnel |
| `procman doctor [name] [-s] [--ai] [--agent <cli>] [-f]` | Diagnose with the offline rule engine or an AI agent; `-f, --fix` applies the suggested fix |
| `procman mcp [-d <dir>]` | Run the MCP server over stdio |
| `procman skill [dir] [-f]` | Install the AI agent skill into a project |
| `procman ui [-a]` | Open the TUI (`-a, --all` opens the global view) |
| `procman upgrade [--tag <vX.Y.Z>]` | Upgrade to the latest (or a specific) release |
| `procman uninstall [-y] [--purge]` | Uninstall and stop processes (`--purge` deletes state and logs) |

---

## TUI Keys (`procman ui`)

| Key | Action |
| :--- | :--- |
| `Tab` | Switch between local project and global dashboard |
| `↑` / `↓` | Select process |
| `Enter` / `Space` | Fullscreen log (`PageUp`/`PageDown`/`Home`/`End` to scroll) |
| `/` / `c` | Filter log lines / clear filter |
| `s` / `x` / `r` | Start / stop / restart |
| `k` | Force kill and free the port |
| `f` / `u` | Open / close Cloudflare tunnel |
| `o` | Show the tunnel URL as a QR code |
| `d` | Remove project from the global registry |
| `q` / `Esc` | Quit |

---

## AI Agent Integration

**MCP server**: lets Antigravity, Cursor, Claude Code, Codex, and Windsurf manage services, read logs, and run diagnostics. Add to your MCP config (`mcp_config.json`, `claude_desktop_config.json`, or `.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "procman": { "command": "procman", "args": ["mcp"] }
  }
}
```
Tools: `procman_status`, `procman_start`, `procman_stop`, `procman_restart`, `procman_logs`, `procman_doctor`, `procman_kill_port`, `procman_ps`, `procman_init`.

**Agent skill**: install usage guidelines into a project with `procman skill`.

---

## Documentation

See [`docs/`](./docs) for architecture, code walkthrough, process flows, and development guides. Release notes are in [CHANGELOG.md](./CHANGELOG.md).

## Support

[![Buy Me A Coffee](https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png)](https://buymeacoffee.com/nhatcoi)

## License

[MIT](./LICENSE) © [nhatcoi](https://github.com/nhatcoi)
