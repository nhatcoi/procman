---
name: feature-workflow
description: >-
  Standard closed-loop development and maintenance lifecycle for procman:
  implementing features, fixing bugs, running verification tests, synchronizing documentation,
  updating agent skills, and recording changelog entries.
---

# `procman` Feature & Bug Fix Workflow Skill

Use this skill whenever implementing a feature, refactoring, or fixing a bug in the `procman` codebase.

---

## 4-Step Standard Lifecycle

```text
[1. Code & Clean] ➔ [2. Test & Verify] ➔ [3. Sync Docs & AI Skills] ➔ [4. Record CHANGELOG]
```

### Step 1: Clean Implementation (Code & Clean)
* **Adhere to Clean Code Standards**:
  - Keep modules focused on a Single Responsibility (SRP).
  - Extract numbers, timeouts, intervals, file extensions, and UI symbols to `const` at the top of the file.
  - Write self-documenting code with meaningful identifiers. Avoid redundant comments.
* **Error Handling**: Use `Result<T>` and `anyhow::Context`. Avoid `.unwrap()` on runtime execution paths.

### Step 2: Quality Gates & Verification (Test & Verify)
* **Compile & Lint**:
  ```bash
  cargo check
  ```
  *Ensure 0 errors and 0 warnings.*
* **Unit & Integration Tests**:
  ```bash
  cargo test
  ```
* **Release Installation & Manual Smoke Test**:
  ```bash
  cargo install --path . --force
  ```

### Step 3: Complete Documentation Synchronization (Docs & AI Skills Sync)
Every user-facing or agent-facing change MUST be updated across:
1. **`README.md`**: CLI reference table, Quick Start steps, TUI keyboard shortcuts.
2. **`docs/feat.md`**: Roadmap tracking (`[x]` in Section 2, summary in Section 1).
3. **`skills/procman/SKILL.md` & `.agents/skills/procman/SKILL.md`**: Agent skill instructions for AI coding assistants.
4. **`docs/03_code_walkthrough_by_file.md`**: Architectural breakdown if new `.rs` files were added.

### Step 4: Record in `CHANGELOG.md`
Add a succinct bullet under `## [Unreleased]` in `CHANGELOG.md`:
* **`Added`**: New CLI subcommands, TUI features, flags, or configuration fields.
* **`Changed`**: Altered existing behavior, upgraded algorithms, UX enhancements.
* **`Fixed`**: Bug fixes, crash prevention, edge-case handling.
* **`Removed`**: Deprecated or removed commands/flags.
* **`Security`**: Vulnerability patches or safe process isolation.
