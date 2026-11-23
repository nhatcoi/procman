# `procman` - Clean Code Guidelines & Architecture Rules

Clean Code standards that apply to the entire `procman` (Rust) codebase.

---

## 1. DRY (Don't Repeat Yourself) & SRP (Single Responsibility Principle)

Each file in `src/` has **exactly one responsibility**:

| Module | Single responsibility |
| :--- | :--- |
| [`src/engine/paths.rs`](../src/engine/paths.rs) | XDG path handling (`~/.local/state`, `~/.local/share`) and SHA-1 project keys. |
| [`src/engine/config.rs`](../src/engine/config.rs) | Locating the config file (walking up directories) and parsing YAML/JSON. |
| [`src/engine/state.rs`](../src/engine/state.rs) | Persisting/reading process state (`state.json`) and liveness checks via Signal 0. |
| [`src/engine/logs.rs`](../src/engine/logs.rs) | Reading logs (`read_tail`) and live log streaming (`follow_log`). |
| [`src/tunnels/cloudflare.rs`](../src/tunnels/cloudflare.rs) | Cloudflare Quick Tunnel lifecycle and URL extraction via regex. |
| [`src/engine/supervisor.rs`](../src/engine/supervisor.rs) | POSIX process orchestration (`setsid`, process group `-pid`, `SIGTERM`/`SIGKILL`, port release). |
| [`src/tui/`](../src/tui) | Interactive TUI dashboard (`ratatui` + `crossterm`). |
| [`src/main.rs`](../src/main.rs), [`src/cli/`](../src/cli) | CLI parser (`clap`) and subcommand routing. |

---

## 2. Zero Magic Values

**Never hardcode** numbers (timeouts, durations, ports) or strings (regexes, extensions, placeholders) inside function bodies.

### Rules
* Every timeout, interval, regex, suffix, filename, and symbol is declared as a **`const` at the top of the file**.
* Constants use `SCREAMING_SNAKE_CASE` and an explicit type.

### Examples
```rust
// ✅ Clean
const SIGTERM_TIMEOUT: Duration = Duration::from_millis(5000);
const SIGKILL_TIMEOUT: Duration = Duration::from_millis(2000);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHELL_BIN: &str = "sh";
const QUICK_TUNNEL_URL_REGEX: &str = r"https://[a-zA-Z0-9-]+\.trycloudflare\.com";
const DEFAULT_PLACEHOLDER: &str = "-";
const DOT_RUNNING: &str = "●";
const DOT_STOPPED: &str = "○";

// ❌ Not clean (magic values)
thread::sleep(Duration::from_millis(150)); // Magic number!
let re = Regex::new(r"https://...").unwrap(); // Magic string!
```

---

## 3. Self-Documenting Code & No Redundant Comments

* **Names must explain themselves**:
  * Name functions with verbs: `find_config_path`, `resolve_names`, `stop_by_pid`, `kill_port`.
  * Name variables clearly: `is_selected`, `should_free_port`, `child_pid`.
* **Do not comment what the code already says**:
  * ❌ *Avoid:* `// 1. Process Table`, `// Poll for keyboard input`, `// Start the process`.
  * ✅ *Comment only the "why", not the "what"*, for OS-specific logic (e.g. why a negative `-pid` targets a POSIX process group).

---

## 4. Robust Error Handling

* **No `.unwrap()` or `panic!()`** in runtime paths.
* Use `Result<T>` and the `?` operator.
* Add meaningful context with `.with_context(|| format!(...))` so developers know exactly where an error occurred (which file, which command).

### Example
```rust
let raw = fs::read_to_string(config_path)
    .with_context(|| format!("Failed to read config file at {:?}", config_path))?;
```

---

## 5. Clean Build & Linter Standards

* `cargo check`, `cargo clippy`, and `cargo test` must all pass with **0 errors and 0 warnings**.
* Remove unused assignments, unused variables, and dead code.
