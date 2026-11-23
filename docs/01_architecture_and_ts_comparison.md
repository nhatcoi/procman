# 01. Architecture & TypeScript vs Rust Comparison

This document explains the core differences in architecture, memory management, process lifecycle, and performance when moving `procman` from **TypeScript (Node.js)** to **Rust**.

---

## 1. Architecture Overview

```
+─────────────────────────────────────────────────────────────+
|                     TypeScript (Node.js)                    |
|                                                             |
|  [procman CLI] ---> [V8 Engine / Node Runtime] ---> [Libuv] |
|                             │                         │     |
|                             ▼                         ▼     |
|                   [Garbage Collector]           [OS Syscalls]
+─────────────────────────────────────────────────────────────+

                              VS

+─────────────────────────────────────────────────────────────+
|                         Rust (Native)                       |
|                                                             |
|  [procman Binary] ────────────────────────────> [OS Syscalls]
|          │                                                  |
|          ▼                                                  |
|   [Zero-Cost Abstractions & RAII (No GC, No VM Runtime)]    |
+─────────────────────────────────────────────────────────────+
```

### Detailed Comparison

| Aspect | TypeScript (Node.js) | Rust (Native Binary) |
| :--- | :--- | :--- |
| **Execution model** | TS source -> compiled to JS -> loaded into the **V8 JIT** VM | Ahead-of-time (AOT) compiled to a native **machine-code** ELF x86_64/ARM64 binary |
| **Memory management** | Automatic via the **Garbage Collector (GC)** | Automatic via **Ownership & RAII** (freed as soon as a value leaves scope, no GC pauses) |
| **Startup latency** | ~100ms – 250ms (V8 runtime init, ES module loading) | **< 2ms** (runs the instant you press Enter) |
| **Distribution size** | `dist/` + `node_modules/` (~40MB – 60MB, thousands of files) | **A single binary** (`procman`, ~3.5MB stripped) |
| **Resource usage (RAM)** | ~30MB – 60MB for the UI process | **~2MB – 4MB** |
| **OS handling (POSIX syscalls)** | Through Node's C++ wrappers (`child_process`, `fs`) | Directly via **libc / the nix crate** (`setsid`, `kill`, `fork/exec`) |

---

## 2. Codebase Module Mapping (TS -> Rust)

Each TypeScript module maps 1:1 to a Rust module:

```
procman (TypeScript)                  procman (Rust)
──────────────────────────────────    ─────────────────────────────────────
src/cli.ts (Commander.js)          -> src/main.rs + src/cli/ (Clap Derive)
src/config.ts (yaml / JSON)        -> src/engine/config.rs (serde_yaml / serde_json)
src/paths.ts (XDG & Crypto)        -> src/engine/paths.rs (dirs + sha1 + hex)
src/state.ts (State JSON & Kill 0) -> src/engine/state.rs (serde_json + nix::sys::signal)
src/logs.ts (Tail & Stream)        -> src/engine/logs.rs (std::fs::File Seek/Read)
src/process-manager.ts (Spawn)     -> src/engine/supervisor.rs (std::process::Command + nix)
src/cloudflare.ts (Tunnel)         -> src/tunnels/cloudflare.rs (cloudflared + regex)
src/ui/App.tsx (Ink + React)       -> src/tui/ (ratatui + crossterm)
```

---

## 3. Why Rust Fits Systems / CLI Tools

1. **Fully self-contained (zero runtime dependencies)**:
   * Users do not need Node.js or `npm install`. Download one `procman` binary, put it on the `PATH`, and it runs on any Linux/macOS machine.
2. **OS-level process group control**:
   * In Node.js, detaching a subshell and signalling a process group (`-PID`) takes several workarounds. In Rust, `std::os::unix::process::CommandExt` and `unsafe { nix::unistd::setsid(); }` assign the session ID and process group exactly.
3. **Strong type, memory, and thread safety**:
   * The `rustc` compiler rules out null-pointer errors, `undefined is not a function`, and data races in multi-threaded state handling and I/O.
