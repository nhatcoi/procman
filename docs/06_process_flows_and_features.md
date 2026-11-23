# 06. Process Flows & Feature Architecture

All flowcharts, sequence diagrams, and state machines for [`procman`](..), with an explanation of how each feature works.

---

## Table of Contents

1. [High-Level Architecture](#1-high-level-architecture)
2. [Project Isolation & XDG Paths](#2-project-isolation--xdg-paths)
3. [Start & Dependency Orchestration](#3-start--dependency-orchestration)
4. [Stop, Kill & Restart](#4-stop-kill--restart)
5. [File Watching & Hot Reload (`procman watch`)](#5-file-watching--hot-reload-procman-watch)
6. [Cloudflare Quick Tunnel & Mobile QR (`forward` & `qr`)](#6-cloudflare-quick-tunnel--mobile-qr-forward--qr)
7. [Real-Time CPU% & RAM Metrics](#7-real-time-cpu--ram-metrics)
8. [Global Registry & `procman ps`](#8-global-registry--procman-ps)
9. [TUI Dashboard Architecture & Loop (`procman ui`)](#9-tui-dashboard-architecture--loop-procman-ui)
10. [Log Reading & Streaming (`procman logs`)](#10-log-reading--streaming-procman-logs)
11. [Update & Uninstall](#11-update--uninstall)
12. [AI Agent Skill Distribution (`procman skill`)](#12-ai-agent-skill-distribution-procman-skill)
13. [Feature, CLI & TUI Key Matrix](#13-feature-cli--tui-key-matrix)

---

## 1. High-Level Architecture

`procman` is **daemonless**. Each child process gets its own session ID (`setsid`) and process group (`-PID`), so it runs independently and uses no system RAM when the TUI is closed.

```mermaid
graph TB
    subgraph CLI_Layer ["1. CLI & User Interaction Layer"]
        CLI["CLI Commands (`main.rs`, `cli/`)"]
        TUI_App["Interactive TUI Dashboard (`tui/`)"]
    end

    subgraph Core_Engine ["2. Core Engine Layer (`engine/`)"]
        ConfigParser["Config Loader & Discovery (`config.rs`)"]
        Supervisor["Process Supervisor (`supervisor.rs`)"]
        Watcher["File Watcher (`watcher.rs`)"]
        Metrics["Tree Metrics Engine (`metrics.rs`)"]
        StateMgr["State Manager (`state.rs`)"]
        Registry["Global Project Registry (`registry.rs`)"]
        LogEngine["Log Seeker & Streamer (`logs.rs`)"]
        Telemetry["Update Checker (`telemetry.rs`, `cli/commands/updater.rs`)"]
    end

    subgraph Networking ["3. Tunneling & Networking (`tunnels/`)"]
        CFTunnel["Cloudflare Quick Tunnel (`cloudflare.rs`)"]
        QRGen["ASCII QR Code Engine (`qr.rs`)"]
    end

    subgraph OS_Kernel ["4. Operating System & Kernel Layer"]
        POSIX_Signals["POSIX Signals (`SIGTERM`, `SIGKILL`, Signal 0)"]
        SysCalls["System Calls (`setsid`, `fork/exec`, TCP Socket Probe)"]
        SysInfo["sysinfo crate (`/proc` metrics)"]
        FS_Storage["XDG File System (`state.json`, `logs/*.log`, `registry.json`)"]
    end

    CLI --> ConfigParser
    CLI --> Supervisor
    CLI --> Watcher
    CLI --> Telemetry
    TUI_App --> Supervisor
    TUI_App --> LogEngine
    TUI_App --> Metrics
    TUI_App --> CFTunnel
    TUI_App --> QRGen

    Supervisor --> StateMgr
    Supervisor --> Registry
    Supervisor --> CFTunnel
    Supervisor --> POSIX_Signals
    Supervisor --> SysCalls

    Watcher --> Supervisor
    Metrics --> SysInfo
    StateMgr --> FS_Storage
    LogEngine --> FS_Storage
    Registry --> FS_Storage
```

---

## 2. Project Isolation & XDG Paths

To avoid state collisions between projects with the same directory or process names, `procman` uses the **XDG Base Directory** standard plus a **SHA-1** hash of the canonicalized absolute path:

```mermaid
flowchart LR
    ConfigPath["/path/to/my-app/procman.yaml"] --> Canonicalize["fs::canonicalize()"]
    Canonicalize --> SHA1["SHA-1 Hashing"]
    SHA1 --> KeyGen["project_key = '{dir_name}-{sha1_first_8}'<br/>(e.g. 'my-app-a8f21bc4')"]

    KeyGen --> StateDir["State Directory:<br/>`~/.local/state/procman/my-app-a8f21bc4/`<br/>• state.json"]
    KeyGen --> LogDir["Log Directory:<br/>`~/.local/share/procman/my-app-a8f21bc4/logs/`<br/>• server.log<br/>• web.log<br/>• server_tunnel.log"]
    KeyGen --> GlobalRegistry["Global Registry:<br/>`~/.local/state/procman/registry.json`"]
```

---

## 3. Start & Dependency Orchestration

[`procman start`](../src/cli/commands/start.rs) resolves the dependency graph (`depends_on`), probes TCP ports for readiness, clears conflicting ports (`free_port`), and detaches the session (`setsid`).

### 3.1. Topological Sort & Cycle Detection

```mermaid
flowchart TD
    StartSort(["Start resolving `depends_on`"]) --> ValidateExist{"Does every dependency<br/>exist in the config?"}
    ValidateExist -- "No" --> ErrUnknown["Error: Unknown dependency"]
    ValidateExist -- "Yes" --> FilterTargets["Select the processes to run<br/>(all, or the target's branch)"]

    FilterTargets --> InitDFS["Initialize 3 DFS states:<br/>0 = Unvisited<br/>1 = Visiting<br/>2 = Visited"]

    InitDFS --> VisitNode["Visit each node with DFS"]
    VisitNode --> CheckState{"State of the child node?"}
    CheckState -- "1 (Visiting)" --> ErrCycle["❌ Cycle detected:<br/>`A -> B -> C -> A`"]
    CheckState -- "0 (Unvisited)" --> Recurse["Recurse into the child"] --> CheckState
    CheckState -- "2 (Visited)" --> Skip["Skip"]

    CheckState --> MarkVisited["Set state = 2 (Visited)<br/>Append the node to `order`"]
    MarkVisited --> DoneOrder(["Return a valid start order"])
```

### 3.2. Full Start Sequence

```mermaid
sequenceDiagram
    autonumber
    actor User as Developer / CLI
    participant CLI as CLI Entry (`start.rs`)
    participant Config as Config Parser (`config.rs`)
    participant Super as Supervisor (`supervisor.rs`)
    participant State as State Manager (`state.rs`)
    participant OS as OS / Kernel (`setsid`)
    participant Probe as TCP Probe (`wait_for_port_ready`)
    participant Tunnel as Cloudflare Tunnel (`cloudflare.rs`)

    User->>CLI: procman start [name] [--forward]
    CLI->>Config: find_config_path() & load_config()
    Config-->>CLI: Config struct (processes, depends_on, ports)
    CLI->>Super: start(config_path, config, name, auto_forward)
    Super->>Super: resolve_dependency_order() -> [db, server, web]

    loop For each process in dependency order
        Super->>State: is_alive(existing_pid)?
        alt Process already running
            State-->>Super: Running -> skip
        else Process not running
            opt Has upstream dependencies
                Super->>Probe: wait_for_port_ready(dep_port, timeout=30s)
                Probe-->>Super: Port ready
            end

            opt `free_port: true`
                Super->>OS: kill_port(port) (fuser -k -9 / lsof)
            end

            Super->>OS: Open log file (append mode)
            Super->>OS: Command::spawn() + pre_exec(setsid)
            OS-->>Super: Child PID (process group = -PID)

            Super->>State: Write ProcEntry to state.json

            opt Downstream processes depend on this one
                Super->>Probe: wait_for_port_ready(own_port, timeout=30s)
                Probe-->>Super: Port open and ready
            end

            opt `forward: true` or `--forward`
                Super->>Tunnel: forward_start(name, port)
                Tunnel-->>Super: Tunnel URL (https://*.trycloudflare.com)
                Super->>State: Write TunnelEntry to state.json
            end
        end
    end

    Super-->>CLI: Update complete
    CLI-->>User: Print the status table
```

---

## 4. Stop, Kill & Restart

`procman` never leaves orphan or zombie processes, because signals go to the whole **process group (`-PID`)**.

```mermaid
stateDiagram-v2
    [*] --> Running: `procman start`

    state "Graceful Stop (`procman stop`)" as Graceful {
        Running --> SendSIGTERM: Send `SIGTERM` to `-PID`
        SendSIGTERM --> PollAlive: Check `is_alive(pid)` (up to 1000ms)
        PollAlive --> StoppedGracefully: Process exited in time
        PollAlive --> SendSIGKILL: Still alive after 1000ms -> send `SIGKILL` to `-PID`
        SendSIGKILL --> StoppedGracefully: Forced stop succeeded
    }

    state "Force Kill (`procman kill` / `procman kill-port`)" as ForceKill {
        Running --> InstantSIGKILL: Send `SIGKILL` to `-PID` & `PID` immediately
        InstantSIGKILL --> FreePort: Call `kill_port(port)` to release the socket
        FreePort --> StoppedForcefully: Fully released
    }

    StoppedGracefully --> UpdateState: Remove from `state.json` & stop related tunnel
    StoppedForcefully --> UpdateState

    UpdateState --> Restarting: For `procman restart` (sleep 200ms)
    Restarting --> Running: Start again in dependency order

    UpdateState --> [*]: For plain Stop/Kill
```

---

## 5. File Watching & Hot Reload (`procman watch`)

[`procman watch`](../src/cli/commands/watch.rs) uses [`watcher.rs`](../src/engine/watcher.rs) and the `notify` crate to detect source changes in real time, with noise filtering and a **350ms debounce**.

```mermaid
flowchart TD
    StartWatch(["Run `procman watch [name]`"]) --> LoadWatchCfg["Read `watch` & `watch_ignore` from YAML"]
    LoadWatchCfg --> RegisterNotify["Register directories with `RecommendedWatcher`<br/>(recursive mode)"]

    RegisterNotify --> EventLoop{"File event loop<br/>(Create / Modify / Remove)"}

    EventLoop --> ReceiveEvent["Receive event from the mpsc channel"]
    ReceiveEvent --> FilterNoise{"`should_ignore_path`:<br/>• Temp files (`.tmp`, `~`, `.log`)<br/>• Defaults: `.git`, `target`, `node_modules`, `dist`...<br/>• Custom: `watch_ignore`"}

    FilterNoise -- "Noise" --> EventLoop
    FilterNoise -- "Valid source file" --> CheckDebounce{"`now - last_restart >= 350ms`?"}

    CheckDebounce -- "Not yet (burst of writes)" --> EventLoop
    CheckDebounce -- "Yes (burst finished)" --> TriggerRestart["Set `last_restart = now`<br/>Print 'File change detected: path'"]

    TriggerRestart --> ExecRestart["Call `supervisor::restart()` for the service"]
    ExecRestart --> EventLoop
```

---

## 6. Cloudflare Quick Tunnel & Mobile QR (`forward` & `qr`)

`procman` can expose a local port (`localhost:PORT`) to the Internet through a **Cloudflare Quick Tunnel** (no account needed) and print a QR code in the terminal for phone scanning.

```mermaid
sequenceDiagram
    autonumber
    actor User as Developer
    participant CLI as CLI / TUI
    participant Tunnel as Cloudflare Engine (`cloudflare.rs`)
    participant OS as Subprocess (`cloudflared`)
    participant State as State Manager (`state.rs`)
    participant QR as QR Generator (`qr.rs`)

    User->>CLI: `procman forward <name>` or the `f` key in the TUI
    CLI->>Tunnel: forward_start(config_path, config, name)
    Tunnel->>OS: Check `cloudflared --version`
    OS-->>Tunnel: OK

    Tunnel->>OS: Command::spawn("cloudflared tunnel --url http://localhost:PORT")<br/>(stdout/stderr redirected to tunnel.log)
    OS-->>Tunnel: Tunnel process PID

    loop Read tunnel.log and match regex (15s timeout)
        Tunnel->>Tunnel: Regex match: r"https://[a-zA-Z0-9-]+\.trycloudflare\.com"
    end

    Tunnel-->>CLI: URL: `https://sample-id.trycloudflare.com`
    CLI->>State: Write `TunnelEntry { pid, url }` to `state.json`

    opt Show QR (the `o` key in the TUI)
        CLI->>QR: generate_qr_ascii(url)
        QR->>QR: Build the 2D matrix with the `qrcode` crate
        QR-->>User: Print the QR in the terminal / TUI modal
    end
```

---

## 7. Real-Time CPU% & RAM Metrics

Unlike tools that measure only the parent PID, [`metrics.rs`](../src/engine/metrics.rs) measures the **entire process tree** (e.g. Node.js workers, Python multiprocessing children, Next.js build workers).

```mermaid
flowchart TD
    StartMetrics["Start measuring for `root_pid`"] --> CheckRoot{"Is `root_pid` alive<br/>in the `sysinfo` table?"}
    CheckRoot -- "No" --> RetNone["Return (None, None)"]

    CheckRoot -- "Yes" --> BuildTreeMap["Iterate all OS processes:<br/>skip OS threads, build a `parent -> children` HashMap"]

    BuildTreeMap --> InitBFS["Initialize BFS queue & visited set:<br/>`queue.push(root_pid)`<br/>`total_cpu = 0.0`, `total_mem_bytes = 0`"]

    InitBFS --> PopNode{"Queue not empty?"}
    PopNode -- "Yes" --> ExtractProc["Pop `current_pid`"]
    ExtractProc --> SumData["Accumulate:<br/>• `total_cpu += proc.cpu_usage()`<br/>• `total_mem_bytes += proc.memory()`"]
    SumData --> PushChildren["Look up children of `current_pid`<br/>Enqueue unvisited child PIDs"]
    PushChildren --> PopNode

    PopNode -- "No (whole tree visited)" --> CalcResult["Convert:<br/>`mem_mb = total_mem_bytes / (1024 * 1024)`"]
    CalcResult --> RetResult(["Return `(Some(total_cpu), Some(mem_mb))`"])
```

---

## 8. Global Registry & `procman ps`

`procman` keeps a central [`registry.json`](../src/engine/registry.rs) of every project it has worked with. [`procman ps`](../src/cli/commands/status.rs) and the **Global TUI** manage services across the whole machine.

```mermaid
flowchart TD
    subgraph Trigger_Auto_Register ["Auto-Registration"]
        AnyCommand["Any command run in a project<br/>(`start`, `stop`, `status`, `ui`...)"] --> ReadReg["Read `~/.local/state/procman/registry.json`"]
        ReadReg --> CheckReg{"Project already registered?"}
        CheckReg -- "No" --> AddReg["Add `ProjectEntry { key, dir, config_path, last_active }`"]
        CheckReg -- "Yes" --> UpdateActive["Set `last_active = Utc::now()`"]
        AddReg --> SaveReg["Save `registry.json`"]
        UpdateActive --> SaveReg
    end

    subgraph Global_Scanning ["System-Wide Scan (`procman ps` / `procman ui --all`)"]
        RunPS["`procman ps` or switching to the Global TUI tab"] --> ScanRegistry["1. Iterate every project in `registry.json`"]
        ScanRegistry --> LoadConfigEach["Read each project's config<br/>(both `● up` and `○ down` services)"]

        LoadConfigEach --> ScanOrphan["2. Scan `~/.local/state/procman/*`<br/>for orphan state outside the registry"]

        ScanOrphan --> MergeRows["Merge rows, measure CPU/RAM via `metrics.rs`"]
        MergeRows --> RenderGlobalTable["Show the machine-wide table:<br/>`PROJECT`, `SERVICE`, `STATUS`, `PID`, `CPU`, `MEM`, `PORT`, `TUNNEL`, `CWD`"]
    end
```

---

## 9. TUI Dashboard Architecture & Loop (`procman ui`)

The terminal UI ([`src/tui/`](../src/tui)) uses **`ratatui`** and **`crossterm`** with **immediate-mode rendering**, non-blocking event handling, and background workers so the UI never blocks.

```mermaid
flowchart TD
    StartTUI(["Run `procman ui`"]) --> SetupTerm["1. Enable raw mode & enter the alternate screen<br/>(`enable_raw_mode`, `EnterAlternateScreen`)"]
    SetupTerm --> InitState["2. Create `AppState` (Local / Global mode)"]

    InitState --> MainLoop{"Main event loop"}

    MainLoop --> PollEvent{"Key event available?<br/>`event::poll(150ms)`"}
    PollEvent -- "Key pressed" --> HandleKey["Handle key (`events.rs`):<br/>• `Tab`: switch Local <-> Global<br/>• `s`/`x`/`k`/`r`: Start/Stop/Kill/Restart<br/>• `Enter`/`Space`: fullscreen log<br/>• `/`: search/filter log<br/>• `o`: QR code modal<br/>• `q`/`Esc`: quit"]

    HandleKey --> SpawnWorker["For I/O actions (Start/Stop/Kill/Forward):<br/>spawn a worker thread<br/>set `pending_actions` ('starting...', 'stopping...')"]
    SpawnWorker --> CheckRefresh

    PollEvent -- "150ms timeout" --> CheckRefresh{"More than 500ms<br/>since the last refresh?"}

    CheckRefresh -- "Due" --> RefreshData["Refresh metrics, process states, log tail"]
    CheckRefresh -- "Not due" --> DrawUI

    RefreshData --> DrawUI["3. Draw (`render.rs`):<br/>• Header & tab bar<br/>• Process table<br/>• Log viewer (search highlight)<br/>• Footer status & key help<br/>• QR modal (if open)"]

    DrawUI --> CheckQuit{"User pressed quit (`q`)?"}
    CheckQuit -- "No" --> MainLoop
    CheckQuit -- "Yes" --> RestoreTerm["4. Disable raw mode & restore the terminal<br/>(`LeaveAlternateScreen`, `disable_raw_mode`)"]
    RestoreTerm --> EndTUI(["Clean exit"])
```

---

## 10. Log Reading & Streaming (`procman logs`)

To show logs instantly without loading gigabyte-sized files into RAM, `procman` seeks backward from the end of the file in [`engine/logs.rs`](../src/engine/logs.rs).

```mermaid
sequenceDiagram
    autonumber
    actor User as Developer
    participant CLI as CLI (`logs.rs`)
    participant LogMgr as Log Engine (`logs.rs`)
    participant File as File System (`<service>.log`)

    User->>CLI: `procman logs <name> -n 50 -f`
    CLI->>LogMgr: read_tail(path, lines=50)

    LogMgr->>File: Open file, get `file_len`
    LogMgr->>File: `seek(SeekFrom::End(-chunk_size))`
    loop Read 4KB chunks backward from the end
        LogMgr->>LogMgr: Count newline characters `\n`
    end
    LogMgr-->>CLI: Exactly the last 50 lines

    CLI-->>User: Print 50 log lines

    opt `-f, --follow` (live streaming)
        loop Follow continuously
            CLI->>File: Read newly appended bytes
            File-->>CLI: New log lines
            CLI-->>User: Print in real time
            CLI->>CLI: Sleep 150ms
        end
    end
```

---

## 11. Update & Uninstall

### 11.1. Automatic Update Check (24h Cache)

```mermaid
flowchart TD
    RunAny["Run any ordinary CLI command"] --> ReadCache["Read the cache at `~/.local/state/procman/update_cache.json`"]
    ReadCache --> CheckTTL{"Cache older than 24h<br/>or missing?"}

    CheckTTL -- "Fresh (< 24h)" --> UseCache["Use the cached version"]
    CheckTTL -- "Stale (> 24h)" --> SpawnCheckThread["Spawn a background thread (non-blocking):<br/>query the GitHub API for the latest release tag"]

    SpawnCheckThread --> SaveCache["Write the result to `update_cache.json` with a new timestamp"]
    SaveCache --> CompareVer{"`latest_version > current_version`?"}
    UseCache --> CompareVer

    CompareVer -- "Newer version" --> ShowBanner["Show a subtle banner after the command:<br/>`💡 A new version of procman is available: vX.Y.Z -> vA.B.C`<br/>`   Run 'procman upgrade' to update automatically.`"]
    CompareVer -- "Up to date" --> NoBanner["Show nothing; CLI is not slowed"]
```

### 11.2. `procman upgrade` & `procman uninstall`

```mermaid
flowchart LR
    subgraph UpgradeFlow ["`procman upgrade`"]
        UpStart(["`procman upgrade`"]) --> FetchRelease["Check GitHub for a new release"]
        FetchRelease --> PromptUp{"Confirm [y/N]<br/>(skipped with `-y`)"}
        PromptUp -- "Yes" --> CargoOrCurl["Prefer `cargo install --git ... --force`<br/>or download a pre-built binary via `install.sh`"]
        CargoOrCurl --> UpDone(["Upgrade complete"])
    end

    subgraph UninstallFlow ["`procman uninstall`"]
        UnStart(["`procman uninstall`"]) --> PromptUn{"Confirm cleanup [y/N]"}
        PromptUn -- "Yes" --> StopAll["Find and stop (`SIGKILL`) all running processes"]
        StopAll --> RemoveBin["Remove the `procman` binary from `$PATH` / `~/.cargo/bin`"]
        RemoveBin --> CheckPurge{"`--purge` set?"}
        CheckPurge -- "Yes" --> DeleteData["Delete data/state directories:<br/>`~/.local/state/procman`<br/>`~/.local/share/procman`"]
        CheckPurge -- "No" --> UnDone(["Uninstall complete"])
        DeleteData --> UnDone
    end
```

---

## 12. AI Agent Skill Distribution (`procman skill`)

So that AI coding assistants (**Antigravity**, **Cursor**, **Claude Code**, **Windsurf**, **Codex**) can control a project's background processes, `procman` embeds the skill definition at compile time:

```mermaid
flowchart TD
    CompileTime["Rust build (`cargo build`)"] --> EmbedMacro["`include_str!('../../../skills/procman/SKILL.md')`<br/>embeds the Markdown in the binary"]

    EmbedMacro --> BinaryReady["Self-contained `procman` binary<br/>(no Internet needed to install the skill)"]

    BinaryReady --> RunSkillCmd["User runs: `procman skill [dir]`"]
    RunSkillCmd --> DetectDir["Resolve the target directory (default: `.agents/skills/procman`)"]

    DetectDir --> CheckExisting{"`SKILL.md` already exists?"}
    CheckExisting -- "Yes" --> CheckMatch{"Content identical to<br/>the embedded copy?"}
    CheckMatch -- "Identical (up to date)" --> InformUpToDate["Print: `✨ procman skill is already up-to-date`"]
    CheckMatch -- "Different / `-f`" --> OverwriteFile["Overwrite with the new content"]

    CheckExisting -- "No" --> CreateParentDir["Create parent directories (`fs::create_dir_all`)"]
    CreateParentDir --> OverwriteFile

    OverwriteFile --> FinishSkill(["Print: `✅ Successfully installed procman skill to .agents/skills/procman/SKILL.md`"])
```

---

## 13. Feature, CLI & TUI Key Matrix

Mapping of **features** to **CLI commands**, **TUI keys**, **`procman.yaml` options**, and the **Rust source files** responsible:

| Feature | CLI command | TUI key (`procman ui`) | `procman.yaml` | Source |
| :--- | :--- | :--- | :--- | :--- |
| **Start processes** | `procman start [name] [-f/--forward]` | `s` | `cmd`, `cwd`, `env`, `port` | [`supervisor.rs`](../src/engine/supervisor.rs), [`start.rs`](../src/cli/commands/start.rs) |
| **POSIX graceful stop** | `procman stop [name]` | `x` | - | [`supervisor.rs`](../src/engine/supervisor.rs), [`stop.rs`](../src/cli/commands/stop.rs) |
| **Force kill & free port** | `procman kill [name]` | `k` | `free_port: true` | [`supervisor.rs`](../src/engine/supervisor.rs) |
| **Free a specific port** | MCP tool `procman_kill_port` | - | `free_port: true` | [`supervisor.rs`](../src/engine/supervisor.rs), [`mcp/`](../src/engine/mcp/mod.rs) |
| **Restart** | `procman restart [name] [-f/--forward]` | `r` | - | [`supervisor.rs`](../src/engine/supervisor.rs) |
| **Status & resources** | `procman status [name]` | Default view | - | [`status.rs`](../src/cli/commands/status.rs), [`metrics.rs`](../src/engine/metrics.rs) |
| **System-wide scan** | `procman ps` / `procman ls` | `Tab` (Global mode) | - | [`status.rs`](../src/cli/commands/status.rs), [`registry.rs`](../src/engine/registry.rs) |
| **Dependencies & readiness** | Automatic on `start` | - | `depends_on: [db, api]` | [`supervisor.rs`](../src/engine/supervisor.rs) |
| **Hot reload on file change** | `procman watch [name]` | - | `watch: true`, `watch_ignore` | [`watcher.rs`](../src/engine/watcher.rs), [`watch.rs`](../src/cli/commands/watch.rs) |
| **Open Cloudflare Quick Tunnel** | `procman forward <name>` | `f` | `forward: true` | [`cloudflare.rs`](../src/tunnels/cloudflare.rs) |
| **Close Cloudflare tunnel** | `procman unforward <name>` | `u` | - | [`cloudflare.rs`](../src/tunnels/cloudflare.rs) |
| **Mobile QR code** | - | `o` (modal) | - | [`qr.rs`](../src/tunnels/qr.rs) |
| **View & stream logs** | `procman logs <name> [-f] [-n]` | Log viewer pane | - | [`logs.rs`](../src/engine/logs.rs), [`logs.rs`](../src/cli/commands/logs.rs) |
| **Search / filter logs** | - | `/` (search), `c` (clear) | - | [`events.rs`](../src/tui/events.rs), [`render.rs`](../src/tui/render.rs), [`logview.rs`](../src/tui/logview.rs) |
| **Fullscreen log** | - | `Enter` / `Space` | - | [`events.rs`](../src/tui/events.rs), [`render.rs`](../src/tui/render.rs) |
| **Remove project from registry** | - | `d` / `Delete` | - | [`registry.rs`](../src/engine/registry.rs), [`actions.rs`](../src/tui/actions.rs) |
| **Update check & self-upgrade** | `procman upgrade` | - | - | [`updater.rs`](../src/cli/commands/updater.rs), [`telemetry.rs`](../src/engine/telemetry.rs) |
| **Clean uninstall** | `procman uninstall [-y] [--purge]` | - | - | [`uninstaller.rs`](../src/cli/commands/uninstaller.rs) |
| **Install AI agent skill** | `procman skill [dir] [-f]` | - | - | [`skill.rs`](../src/cli/commands/skill.rs) |
| **Diagnose & fix** | `procman doctor [name] [-s] [--ai] [-f]` | - | - | [`doctor.rs`](../src/cli/commands/doctor.rs), [`doctor/`](../src/engine/doctor/mod.rs) |
| **Generate config** | `procman init [dir] [--ai] [-y]` | - | - | [`init.rs`](../src/cli/commands/init.rs), [`init/`](../src/engine/init/mod.rs) |
| **MCP server for AI agents** | `procman mcp [-d <dir>]` | - | - | [`mcp.rs`](../src/cli/commands/mcp.rs), [`mcp/`](../src/engine/mcp/mod.rs) |
