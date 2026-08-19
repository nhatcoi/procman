---
name: procman
description: >-
  Manage and run background development processes, servers, and workers defined in procman.yaml.
  Use this skill whenever setting up multi-service projects, creating or editing procman.yaml,
  starting/stopping background services, checking service status, reading process logs,
  or forwarding local ports via Cloudflare tunnels.
---

# `procman` Process Manager Skill

`procman` is a lightweight, background process manager for managing development processes (servers, workers, watchers) defined in a project config file (`procman.yaml`). It runs processes in detached background shells, streams logs to isolated files, performs PID liveness checks, and supports Cloudflare quick tunnels for public URL exposure.

---

## 1. Creating & Configuring `procman.yaml`

Place `procman.yaml` (or `.procman.yaml` / `procman.json`) in the root directory of the project.

### Configuration Schema

```yaml
# procman.yaml
processes:
  <process_name>:
    cmd: string               # Required: Shell command to execute
    cwd: string               # Optional: Working directory relative to procman.yaml (default: ".")
    port: number              # Optional: Port number (required if using `procman forward` or `forward: true`)
    forward: boolean          # Optional: Auto-start Cloudflare tunnel when started (default: false)
    free_port: boolean        # Optional: Force-kill any process occupying this port before running (default: false)
    env:                      # Optional: Key-value map of environment variables
      KEY: "value"
```

### Full Example

```yaml
processes:
  api:
    cmd: "go run ./cmd/server"
    cwd: "./backend"
    port: 8080
    env:
      PORT: "8080"
      ENV: "development"
      DATABASE_URL: "postgres://user:pass@localhost:5432/app"

  web:
    cmd: "npm run dev"
    cwd: "./frontend"
    port: 3000

  worker:
    cmd: "python -m worker.main"
    cwd: "./services/worker"
    env:
      CONCURRENCY: "4"
```

---

## 2. CLI Command Reference

Execute `procman` commands from within the project directory (or any subfolder; `procman` automatically walks up parent directories to find the config file).

### Service Control

| Action | Command | Description |
| :--- | :--- | :--- |
| **Start All** | `procman start` | Starts all configured processes in the background |
| **Start One** | `procman start <name>` | Starts only the specified process |
| **Stop All** | `procman stop` | Gracefully terminates all processes and active tunnels |
| **Stop One** | `procman stop <name>` | Stops a single process |
| **Force Kill** | `procman kill [name]` | Force kills processes with `SIGKILL` and frees their ports |
| **Kill Port** | `procman kill-port <port>` | Force kills whatever process is occupying a specific port |
| **Restart All** | `procman restart` | Stops and re-starts all processes |
| **Restart One** | `procman restart <name>` | Restarts a single process |

### Status & Inspection

| Action | Command | Description |
| :--- | :--- | :--- |
| **Status** | `procman` or `procman status` | Displays tabular status for current project (or auto-lists global processes if outside a repo) |
| **Global Process List** | `procman ps` or `procman ls` | Lists all active processes across all projects on the entire system |
| **View Logs** | `procman logs <name>` | Prints the last 100 lines of log |
| **Tail N Lines** | `procman logs <name> -n 50` | Prints the last 50 lines of log |
| **Follow Logs** | `procman logs <name> -f` | Real-time streaming log output (`tail -f`) |
| **QR Code** | `procman qr <name>` | Renders a terminal QR code for the process's active tunnel or local URL |
| **Upgrade** | `procman upgrade` | Checks and self-upgrades procman to the latest release |
| **Uninstall** | `procman uninstall` | Completely uninstalls procman binary and stops processes (`-p` to purge) |

### Public Port Forwarding (Cloudflare Tunnels)

*Requires `cloudflared` installed on PATH and a `port` defined in `procman.yaml`.*

| Action | Command | Description |
| :--- | :--- | :--- |
| **Forward Port** | `procman forward <name>` | Spawns a Cloudflare tunnel, prints the URL and ASCII QR code |
| **Stop Forward** | `procman forward-stop <name>` | Stops the active tunnel for that process |

### Interactive TUI Dashboard

```bash
# Open local project dashboard (or auto-global if outside a repo)
procman ui

# Open directly in Global System Dashboard
procman ui --all
```
*Keyboard shortcuts: `Tab` switch between Local/All Projects, `↑`/`↓` navigate, `Enter`/`Space` fullscreen log, `/` search/filter logs, `c` clear filter, `s` start (Local & Global), `x` stop, `k` force-kill, `r` restart, `f` forward, `o` QR popup modal, `u` unforward, `d` unregister project, `q` quit.*

---

## 3. Agent Operating Guidelines & Workflows

When working in projects with multiple processes or when asked to run background servers:

### A. Setting Up a New Project
1. Identify the services (e.g., frontend, backend, workers, queues).
2. Generate a clean `procman.yaml` in the project root with exact `cmd`, `cwd`, and `port` values.
3. Verify with `procman status`.

### B. Starting Services Non-Blockingly
Instead of running long-lived blocking commands (like `npm run dev` or `go run .`) directly in agent commands:
1. Run `procman start` or `procman start <name>`.
2. Inspect the status with `procman status`.
3. Verify startup health by inspecting recent logs:
   ```bash
   procman logs <name> -n 30
   ```

### C. Troubleshooting Service Failures
1. If status shows `down` shortly after starting:
   - Check `procman logs <name>` for exit errors or missing environment variables.
   - Check whether another process is already bound to the target `port` (`lsof -i :<port>`).
2. After modifying code/config, reload using `procman restart <name>`.

### D. File Storage Locations
* State metadata is stored at: `~/.local/state/procman/<projectKey>/state.json`
* Raw log files are stored at: `~/.local/share/procman/<projectKey>/logs/<name>.log`
