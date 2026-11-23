# 04. Development & Cargo Guide

This document collects the basic and advanced Cargo commands you need to work on and extend `procman`.

---

## 1. Everyday Cargo Commands

### Fast syntax check (no binary produced)
```bash
cargo check
```
*Very fast (a few hundred ms). Run it continuously while coding to catch type errors.*

### Build & run in debug mode
```bash
cargo run -- status
cargo run -- start web
cargo run -- ui
```

### Optimized release build
```bash
cargo build --release
```
*The binary lands at `target/release/procman`: small (~2.7MB) and fully optimized.*

### Install into the system (`~/.cargo/bin/procman`)
```bash
cargo install --path .
```
*Builds a release binary and copies it to `~/.cargo/bin/`, so you can run `procman` from any directory.*

---

## 2. Code Quality Tools (Linter & Formatter)

### Auto-format the source
```bash
cargo fmt
```

### Lint and get optimization hints (Clippy)
```bash
cargo clippy
```
*Clippy is Rust's official linter and a good way to learn idiomatic Rust.*

---

## 3. Project Layout

```text
procman/
├── Cargo.toml          # Dependencies & build configuration
├── Cargo.lock          # Pinned dependency versions
├── README.md           # Quick-start guide
├── procman.example.yaml# Example project config
├── src/                # Rust source
│   ├── main.rs         # Entry point
│   ├── cli/            # clap args, command handlers, output formatting
│   ├── engine/         # config, paths, state, supervisor, logs, metrics,
│   │                   # registry, watcher, telemetry, doctor, init, mcp
│   ├── tunnels/        # Cloudflare tunnel & QR code
│   └── tui/            # Ratatui terminal dashboard
├── docs/               # Project documentation
│   ├── 01_architecture_and_ts_comparison.md
│   ├── 02_rust_concepts_guide.md
│   ├── 03_code_walkthrough_by_file.md
│   ├── 04_development_and_cargo_guide.md
│   ├── 05_clean_code_rules.md
│   ├── 06_process_flows_and_features.md
│   └── feat.md
└── skills/             # Agent skills for AI assistants
    └── procman/
        └── SKILL.md
```
