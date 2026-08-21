use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

pub const SKILL_FILENAME: &str = "SKILL.md";
pub const EMBEDDED_SKILL_MD: &str = include_str!("../../../skills/procman/SKILL.md");

pub fn execute(dir: &str, force: bool) -> Result<()> {
    let raw_path = Path::new(dir);
    let target_file: PathBuf = if raw_path.extension().and_then(|ext| ext.to_str()) == Some("md") {
        raw_path.to_path_buf()
    } else {
        raw_path.join(SKILL_FILENAME)
    };

    println!("📥 Installing procman AI agent skill into {}...", target_file.display());

    if let Some(parent) = target_file.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create destination directory: {}", parent.display()))?;
        }
    }

    if target_file.exists() && !force {
        if let Ok(existing_content) = fs::read_to_string(&target_file) {
            if existing_content == EMBEDDED_SKILL_MD {
                println!("✨ procman skill is already up-to-date at {}!", target_file.display());
                println!("   Your AI agent (Antigravity, Cursor, Claude Code, Codex) is ready to manage project processes.");
                return Ok(());
            }
        }
    }

    fs::write(&target_file, EMBEDDED_SKILL_MD)
        .with_context(|| format!("Failed to write skill file to {}", target_file.display()))?;

    println!("✅ Successfully installed procman skill to {}!", target_file.display());
    println!("   Your AI agent (Antigravity, Cursor, Claude Code, Codex) can now autonomously manage project processes.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_skill_content_not_empty() {
        assert!(!EMBEDDED_SKILL_MD.is_empty());
        assert!(EMBEDDED_SKILL_MD.contains("name: procman"));
        assert!(EMBEDDED_SKILL_MD.contains("procman.yaml"));
    }

    #[test]
    fn test_skill_execution_in_temp_dir() {
        let temp_dir = std::env::temp_dir().join(format!("procman_skill_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let target_dir_str = temp_dir.to_str().unwrap();
        let res = execute(target_dir_str, false);
        assert!(res.is_ok());

        let target_file = temp_dir.join(SKILL_FILENAME);
        assert!(target_file.exists());

        let content = fs::read_to_string(&target_file).unwrap();
        assert_eq!(content, EMBEDDED_SKILL_MD);

        // Test running again without force (should succeed with up-to-date check)
        let res_repeat = execute(target_dir_str, false);
        assert!(res_repeat.is_ok());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
