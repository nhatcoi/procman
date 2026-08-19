# Feature & Bug Fix Workflow Rules for `procman`

Whenever implementing a new feature, modifying existing behavior, or fixing a bug in `procman`, the Agent MUST strictly follow this 4-step closed-loop lifecycle:

---

## Step 1: Clean Implementation (Code & Clean)
* Adhere to `.agents/rules/clean_code.md`.
* **Zero Magic Values**: Extract numbers, timeouts, intervals, file extensions, and UI symbols to `const` definitions at the top of the file.
* **SRP & Modularization**: Create a dedicated module in `src/` if introducing a distinct domain (e.g. `qr.rs`, `updater.rs`, `uninstaller.rs`).
* **Error Handling**: Use `Result<T>` and `anyhow::Context`. Avoid `.unwrap()` on runtime paths.

---

## Step 2: Quality Gates & Verification (Test & Verify)
* Run `cargo check` and ensure **0 errors and 0 warnings**.
* Run `cargo test` and verify all tests pass.
* Verify CLI / TUI execution behavior in terminal before declaring completion.

---

## Step 3: Complete Documentation Synchronization (Docs & AI Skills Sync)
Never leave documentation out of sync. You MUST update all of the following in the same task:
1. **User Documentation (`README.md`)**:
   - Update the Quick Start guide or CLI reference table.
   - Update the TUI keyboard shortcuts table if TUI shortcuts changed.
2. **Roadmap & Progress Tracking (`docs/feat.md`)**:
   - Mark the completed item as `[x]` in Section 2 (Planned Features / Backlog).
   - Summarize the new capability in Section 1 (Current Features).
3. **AI Agent Skills**:
   - `skills/procman/SKILL.md`: **Chỉ dành cho skill phân phối công khai của `procman`** (cho người dùng ngoài tải về dùng).
   - `.agents/skills/`: Thư mục skills nội bộ phục vụ phát triển dự án (`procman/`, `feature-workflow/`, `release/`).
   - Cập nhật bảng lệnh, phím tắt TUI và cờ CLI để AI assistant nắm bắt tính năng mới.
4. **Architecture Walkthrough (`docs/03_code_walkthrough_by_file.md`)**:
   - Add entry if a new source file was added to `src/`.

---

## Step 4: Record User-Facing Changes (`CHANGELOG.md`)
Record all user-facing changes under the `## [Unreleased]` section in `CHANGELOG.md`:
* **`Added`**: New CLI subcommands, TUI features, flags, or configuration fields.
* **`Changed`**: Altered existing behavior, upgraded algorithms, UX enhancements.
* **`Fixed`**: Bug fixes, crash prevention, edge-case handling.
* **`Removed`**: Deprecated or removed commands/flags.
* **`Security`**: Vulnerability patches or safe process isolation.
