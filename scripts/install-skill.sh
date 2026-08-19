#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:-.agents/skills/procman}"
mkdir -p "$TARGET_DIR"

echo "📥 Installing procman AI agent skill into $TARGET_DIR/SKILL.md..."
curl -fsSL https://raw.githubusercontent.com/nhatcoi/procman/main/skills/procman/SKILL.md -o "$TARGET_DIR/SKILL.md"

echo "✅ Successfully installed procman skill to $TARGET_DIR/SKILL.md!"
echo "   Your AI agent (Antigravity, Cursor, Claude Code) can now autonomously manage project processes."
