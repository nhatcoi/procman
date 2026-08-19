# procman

> **A high-performance, daemonless background process manager and runner for local development written in Rust.**

`procman` allows you to manage multi-service development stacks (backend APIs, frontend dev servers, background workers, watchers) defined in a single `procman.yaml` file. It features sub-millisecond startups, realtime CPU/RAM monitoring, Cloudflare quick tunneling with mobile QR codes, and an interactive TUI dashboard.

---

## 🚀 Installation & Setup

### ⚡ One-Line Online Install (Recommended)

#### Option 1: Via Quick Shell Script (Auto-installs Latest Release)
```bash
curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/install.sh | sh
```
*To install a specific version (e.g. `v0.1.1`):*
```bash
VERSION=v0.1.1 curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/install.sh | sh
```

#### Option 2: Via Cargo with Release Tag
```bash
# Install the latest released version (v0.1.1)
cargo install --git https://github.com/nhatcoi/procman.git --tag v0.1.1 --force

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

## 🤖 AI Agent Skill Setup

Enable AI coding assistants (**Antigravity**, **Cursor**, **Claude Code**, **Windsurf**) to autonomously manage background processes in your project:

```bash
# Option 1: Via procman CLI (Instant & offline)
procman skill

# Option 2: Via one-line curl command
curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/skills/procman/SKILL.md -o .agents/skills/procman/SKILL.md --create-dirs
```

---

## ⚡ Quick Start Guide

### Step 1: Create `procman.yaml`

Create a `procman.yaml` (or `procman.json`) in the root directory of your project:

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
    forward: true   # Auto-spawns a Cloudflare tunnel and gives a public URL

  # Background Queue Worker
  worker:
    cmd: "python -m worker.main"
    cwd: "./services/worker"
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
| `procman ps` / `procman ls` | **List all active processes across all projects on the entire machine** |
| `procman start [name] [-f]` | Start all or specific process (`-f, --force` to free ports) |
| `procman stop [name]` | Gracefully stop all or specific process (`SIGTERM`) |
| `procman restart [name] [-f]` | Restart all or specific process |
| `procman kill [name]` | Force kill (`SIGKILL`) and immediately free ports |
| `procman kill-port <port>` | Force kill whatever process is occupying a specific port |
| `procman logs <name> [-f] [-n 100]` | View or follow logs for a process |
| `procman forward <name>` | Expose port via Cloudflare Quick Tunnel and print QR code |
| `procman forward-stop <name>` | Stop active Cloudflare Quick Tunnel |
| `procman qr <name>` | Print ASCII QR code in terminal for mobile scanning |
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

## 📄 License & Changelog

- [Changelog](./CHANGELOG.md)
- [MIT License](./LICENSE)

MIT © [nhatcoi](https://github.com/nhatcoi)
