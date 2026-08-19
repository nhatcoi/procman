# Clean Code Rules for `procman` (Rust)

When developing or modifying `procman`:
1. **DRY & SRP**: Each file in `src/` has a single responsibility. Avoid duplicate logic.
2. **Zero Magic Values**: Declare all timeouts, polling intervals, regex patterns, UI symbols, file extensions, and placeholders as `const` at top of file.
3. **Self-Documenting Code**: Avoid redundant/bloated comments that state the obvious. Use clean, expressive naming.
4. **Error Handling**: Use `Result<T>` and `anyhow::Context`. Never `.unwrap()` in runtime paths.
5. **Zero Warnings**: Ensure `cargo check` and `cargo test` pass with 0 warnings.
