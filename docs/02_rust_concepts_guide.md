# 02. Core Rust Concepts Used in `procman`

A hands-on guide that explains the most important Rust concepts through `procman`'s own source code.

---

## 1. Ownership & Borrowing

### `String` vs `&str`, `PathBuf` vs `&Path`

Rust data comes in two forms: **owned heap data** and **borrowed slices**.

* **`String` / `PathBuf`**: own their data on the heap, can grow, and are freed automatically when they leave scope.
* **`&str` / `&Path`**: read-only references to existing data; they allocate nothing.

#### Example from `src/engine/config.rs`:
```rust
// Takes &Path (a borrow, no allocation) and returns Result<Config>
pub fn load_config(config_path: &Path) -> Result<Config> {
    let raw = fs::read_to_string(config_path)?; // read_to_string accepts &Path
    // raw is a String (owns the file contents)
    let config: Config = serde_yaml::from_str(&raw)?; // from_str borrows &raw
    Ok(config)
}
```

> **Rule of thumb:** when a function only reads its input, accept `&str` or `&Path` instead of `String` or `PathBuf` to avoid needless `.clone()` calls.

---

## 2. Error Handling with `anyhow` & the `?` Operator

In TypeScript, errors are thrown with `throw new Error(...)` and caught with `try { ... } catch (e) { ... }`.
In Rust, an error is an explicit value of type **`Result<T, E>`**, which is either `Ok(T)` or `Err(E)`.

### The `?` operator (early return on error)
Put `?` after a call that returns a `Result`:
* On `Ok(value)`: evaluates to `value` and execution continues.
* On `Err(error)`: returns that `Err` from the current function immediately.

#### Comparison
**TypeScript:**
```typescript
function getDir(path: string): string {
  try {
    const abs = fs.realpathSync(path);
    return abs;
  } catch (err) {
    throw new Error(`Failed to resolve path: ${err.message}`);
  }
}
```

**Rust (`src/engine/paths.rs`):**
```rust
use anyhow::{Context, Result};

pub fn project_key(config_path: &Path) -> Result<String> {
    // fs::canonicalize returns std::io::Result<PathBuf>
    // .with_context(...) attaches a descriptive message
    // `?` returns the Err automatically on failure
    let abs = fs::canonicalize(config_path)
        .with_context(|| format!("Failed to canonicalize config path {:?}", config_path))?;

    let parent = abs.parent().unwrap_or(&abs);
    // ...
    Ok(format!("{}-{}", base, short_hash))
}
```

---

## 3. Data Structures with `Serde` Derive

`serde` serializes (struct -> JSON/YAML) and deserializes (JSON/YAML -> struct) at compile time, with no runtime reflection cost.

#### Example from `src/engine/config.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDef {
    pub cmd: String,
    pub cwd: Option<String>,                   // like cwd?: string in TS
    pub env: Option<HashMap<String, String>>,  // like Record<string, string>
    pub port: Option<u16>,                     // like port?: number
    pub forward: Option<bool>,                 // None if absent from the YAML
    pub tunnel: Option<bool>,
}
```

* `Option<T>` is a value that is either present (`Some(value)`) or absent (`None`). It replaces `null` and `undefined`.

---

## 4. Systems Programming (POSIX Syscalls) with `nix`

A major strength of Rust is direct access to the Linux/Unix kernel.

### A. Checking whether a process is alive (Signal 0 probe)
On Linux, sending signal `0` to a PID does not terminate it, but the OS returns success if the PID exists and you may signal it.

#### In `src/engine/state.rs`:
```rust
use nix::sys::signal::kill;
use nix::unistd::Pid;

pub fn is_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // None represents Signal 0 in the `nix` crate
    kill(Pid::from_raw(pid), None).is_ok()
}
```

### B. Creating an independent process group (`setsid`)
When you run `cmd = "npm run dev"`, Node spawns several child processes. Killing only the parent PID leaves the children orphaned and still holding the port.
`setsid()` puts the parent and all its children in their own **process group**:

#### In `src/engine/supervisor.rs`:
```rust
use std::os::unix::process::CommandExt;

let mut cmd = Command::new("sh");
cmd.arg("-c").arg(&def.cmd);

// `pre_exec` runs between fork() and exec()
unsafe {
    cmd.pre_exec(|| {
        nix::unistd::setsid().map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
        Ok(())
    });
}
let child = cmd.spawn()?;
```

To stop the whole process tree:
```rust
// A negative PID (-pid) signals the ENTIRE process group
let _ = kill(Pid::from_raw(-pid), Signal::SIGTERM);
```

---

## 5. Terminal UI with `ratatui`

`ratatui` uses **immediate-mode rendering**:
1. The terminal enters raw mode (`crossterm::terminal::enable_raw_mode()`).
2. On every frame, the draw function renders the whole UI from the current state.
3. There is no DOM-like tree to manage, unlike React/Ink.

#### Simplified event loop (see `src/tui/mod.rs`):
```rust
loop {
    // 1. Refresh data
    rows = supervisor::status(&config_path, &config, None)?;

    // 2. Draw the UI
    terminal.draw(|f| {
        // Layout: table on top, logs in the middle, key help at the bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(8), Constraint::Length(1)])
            .split(f.area());

        f.render_widget(table, chunks[0]);
        f.render_widget(log_box, chunks[1]);
    })?;

    // 3. Non-blocking key poll
    if event::poll(Duration::from_millis(150))? {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break, // Quit
                KeyCode::Char('s') => { /* Start process */ },
                KeyCode::Char('f') => { /* Forward tunnel */ },
                // ...
            }
        }
    }
}
```
