# procman

[![Views](https://hits.sh/github.com/nhatcoi/procman.svg?label=views&color=007ec6)](https://hits.sh/github.com/nhatcoi/procman/)
[![Downloads](https://img.shields.io/github/downloads/nhatcoi/procman/total?label=downloads&color=4c1)](https://github.com/nhatcoi/procman/releases)
[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-Support-orange?style=flat&logo=buy-me-a-coffee)](https://buymeacoffee.com/nhatcoi)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> **A high-performance, daemonless background process manager and runner for local development written in Rust.**

`procman` allows you to manage multi-service development stacks (backend APIs, frontend dev servers, background workers, watchers) defined in a single `procman.yaml` file. It features sub-millisecond startups, realtime CPU/RAM monitoring, Cloudflare quick tunneling with mobile QR codes, and an interactive TUI dashboard.

---

## 🚀 Installation & Setup

### ⚡ One-Line Online Install (Recommended - No Rust/Cargo required)

Automatically downloads and installs the pre-compiled standalone binary for your OS (Linux x86_64/ARM64, macOS Intel/Apple Silicon) in seconds:

#### Linux & macOS (sh / curl)
```bash
curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/install.sh | sh
```
*To install a specific version (e.g. `v0.1.3`):*
```bash
VERSION=v0.1.3 curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/install.sh | sh
```

#### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/nhatcoi/procman/main/install.ps1 | iex
```

#### Option 2: Via Cargo with Release Tag
```bash
# Install the latest released version (v0.1.3)
cargo install --git https://github.com/nhatcoi/procman.git --tag v0.1.3 --force

# Or install bleeding-edge directly from main branch
cargo install --git https://github.com/nhatcoi/procman.git --force
```

---

### 📦 Build from Local Source

```bash
# Clone the repository
git clone https://github.com/nhatcoi/procman.git
cd procman

# Build in release mode and install to ~/.cargo/bin
cargo install --path . --force
```

### 3. Verify PATH

Ensure `~/.cargo/bin` is in your shell `$PATH`:

```bash
# Add to ~/.bashrc or ~/.zshrc if not already present
export PATH="$HOME/.cargo/bin:$PATH"

# Verify installation
procman --version
```

---

## 🤖 AI Agent Integration (MCP & Skills)

### 1. Model Context Protocol (MCP) Server (`procman mcp`)

Directly connect AI assistants (**Antigravity**, **Cursor**, **Claude Code**, **Codex**, **Windsurf**) to control background services, inspect real-time logs, query system liveness, and run AI diagnostics via standard JSON-RPC 2.0 stdio:

Add to your MCP settings (`mcp_config.json`, `claude_desktop_config.json`, or `.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "procman": {
      "command": "procman",
      "args": ["mcp"]
    }
  }
}
```

### 2. Autonomous Agent Skill Setup (`procman skill`)

Install procman's workflow guidelines and execution cheatsheet into your local project:

```bash
# Option 1: Via procman CLI (Instant & offline)
procman skill

# Option 2: Via one-line curl command
curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/skills/procman/SKILL.md -o .agents/skills/procman/SKILL.md --create-dirs
```

---

## ⚡ Quick Start Guide

### Step 1: Initialize Configuration (`procman init`)

Auto-detect project frameworks, services, ports, and manifests (or generate manually):

```bash
# Auto-scan project & generate procman.yaml (Heuristic or local AI Agent)
procman init

# Or force AI Agent generation directly
procman init --ai
```

Sample generated `procman.yaml` (or `procman.json`):

```yaml
processes:
  # Backend API Server
  server:
    cmd: "npm run dev:server"
    port: 3001
    free_port: true # Auto-kills any process occupying port 3001 before starting
    env:
      PORT: "3001"
      NODE_ENV: "development"

  # Frontend Web Client
  web:
    cmd: "npm run dev"
    cwd: "./web"
    port: 5173
    depends_on:
      - server      # Waits for server port 3001 to open before starting web
    forward: true   # Auto-spawns a Cloudflare tunnel and gives a public URL

  # Background Queue Worker
  worker:
    cmd: "python -m worker.main"
    cwd: "./services/worker"
    depends_on:
      - server
    env:
      CONCURRENCY: "4"
```

### Step 2: Start Your Services

```bash
# Start all processes in the background
procman start

# Or start only a specific process
procman start server

# Or start with force port-clearing
procman start server --force
```

### Step 3: Check Status & Realtime Metrics

```bash
procman status
```

Output:
```text
NAME    STATUS   PID      CPU      MEM      UPTIME   PORT   TUNNEL
server  up       124581   0.8%     38MB     14m20s   3001   -
web     up       124589   0.2%     54MB     14m18s   5173   https://sample-domain.trycloudflare.com
worker  up       124602   0.0%     22MB     14m15s   -      -
```

### Step 4: Scan QR Code on Mobile

For services exposed with `forward: true` or `procman forward <name>`:

```bash
# Print mobile-scannable ASCII QR code in your terminal
procman qr web
```

### Step 5: View Logs

```bash
# View last 100 lines of logs
procman logs server

# Live stream logs (like tail -f)
procman logs server -f

# View last 50 lines
procman logs server -n 50
```

### Step 6: Interactive TUI Dashboard

```bash
procman ui
```

### Step 7: Live File Watching & Auto-Reload (`procman watch`)

Automatically restart processes whenever source code files change with built-in 350ms debouncing and noise filtering:

```bash
# Watch all configured processes
procman watch

### Step 8: Diagnose & Fix Crashes (`procman doctor`)

Choose between **Spot Check** (instant offline Rule Engine) or **AI Check** (deep, visual AI Agent analysis across processes):

```bash
# Interactively choose between [1] Spot Check or [2] AI Check
procman doctor

# Run fast offline Spot Check directly
procman doctor -s

# Delegate deep system / process health analysis to local AI Agent CLI (claude, agy, gemini, ollama)
procman doctor --ai
procman doctor api --ai --agent claude

# Automatically apply the suggested remediation command
procman doctor api --fix
```

---

## ⌨️ TUI Keyboard Shortcuts (`procman ui`)

| Key | Action | Description |
| :--- | :--- | :--- |
| `Tab` | **Switch Mode** | **Toggle between Local Project View and Global Dashboard (All Projects)** |
| `↑` / `↓` | **Navigate** | Select between processes |
| `Enter` / `Space` | **Zoom Log** | **Toggle Fullscreen Log View** with scroll history (`PageUp`/`PageDown`/`↑`/`↓`/`Home`/`End`) |
| `/` | **Search / Filter** | **Type keywords to filter log lines live** and highlight matching terms |
| `c` | **Clear Filter** | Clear the active search filter |
| `s` | **Start** | Start the selected process in the background (works in both Local & Global view) |
| `x` | **Stop** | Gracefully stop the selected process (`SIGTERM`) |
| `k` | **Force Kill** | Cưỡng chế kill (`SIGKILL`) and immediately free the occupied port |
| `r` | **Restart** | Stop and start the selected process |
| `f` | **Forward** | Spawn a Cloudflare Quick Tunnel for the process's port |
| `o` | **QR Code** | **Open popup modal with ASCII QR Code** to scan with phone camera |
| `u` | **Unforward** | Stop the Cloudflare tunnel |
| `d` | **Unregister** | Remove selected project from the Global Dashboard registry |
| `q` / `Esc` | **Quit** | Exit the TUI dashboard |

---

## 🛠️ Complete CLI Reference

| Command | Description |
| :--- | :--- |
| `procman` / `procman status` | Show status of current project (or auto-lists global processes if outside a repo) |
| `procman init [dir] [--ai] [-y]` | **Auto-scan codebase & generate optimal `procman.yaml` (Heuristic or AI)** |
| `procman ps` / `procman ls` | **List all active processes across all projects on the entire machine** |
| `procman start [name] [-f]` | Start all or specific process (`-f, --force` to free ports) |
| `procman stop [name]` | Gracefully stop all or specific process (`SIGTERM`) |
| `procman restart [name] [-f]` | Restart all or specific process |
| `procman kill [name]` | Force kill (`SIGKILL`) and immediately free ports |
| `procman kill-port <port>` | Force kill whatever process is occupying a specific port |
| `procman logs <name> [-f] [-n 100]` | View or follow logs for a process |
| `procman doctor [name] [-s] [--ai] [-f]` | **Diagnose processes: Spot Check rule scanner or AI Check health analysis & fixes** |
| `procman forward <name>` | Expose port via Cloudflare Quick Tunnel and print QR code |
| `procman forward-stop <name>` | Stop active Cloudflare Quick Tunnel |
| `procman qr <name>` | Print ASCII QR code in terminal for mobile scanning |
| `procman mcp [-d, --dir <dir>]` | **Run Model Context Protocol (MCP) server for AI agents over stdio JSON-RPC** |
| `procman skill` | Install AI agent skill (`.agents/skills/procman/SKILL.md`) into current project |
| `procman upgrade` / `update` | Check for updates and upgrade procman to the latest release (`-y` to skip prompt) |
| `procman uninstall` | Completely uninstall procman and stop background processes (`-p, --purge` to delete state/logs) |
| `procman ui [-a, --all]` | Launch interactive TUI dashboard (`-a, --all` to open Global view directly) |


---

## ⚙️ `procman.yaml` Configuration Schema

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `cmd` | `string` | **Yes** | Shell command to execute (e.g. `"npm run dev"`, `"go run ."`) |
| `cwd` | `string` | No | Working directory relative to `procman.yaml` (default: `"."`) |
| `port` | `number` | No | Local listening port number (required for tunneling) |
| `free_port` | `boolean` | No | Automatically kills any process occupying this port before starting |
| `forward` | `boolean` | No | Automatically spawns a Cloudflare tunnel on start (default: `false`) |
| `env` | `map` | No | Key-value map of environment variables |

---

## 💖 Support the Project

If you find `procman` helpful and want to support its active development:

<a href="https://buymeacoffee.com/nhatcoi" target="_blank">
  <img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" height="42">
</a>

---

## 📄 License & Changelog

- [Changelog](./CHANGELOG.md)
- [MIT License](./LICENSE)

MIT © [nhatcoi](https://github.com/nhatcoi)
