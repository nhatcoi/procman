#!/usr/bin/env bash
set -euo pipefail

NEW_VERSION="${1:-}"

if [ -z "$NEW_VERSION" ]; then
  echo "❌ Error: Missing version argument."
  echo "Usage: ./scripts/release.sh <version> (e.g. 0.3.0)"
  exit 1
fi

# Clean version string (remove leading 'v' if provided)
NEW_VERSION="${NEW_VERSION#v}"

echo "=========================================="
echo "🚀 Preparing Release: v$NEW_VERSION"
echo "=========================================="

# 1. Quality Gates
echo "\n🔍 [1/5] Running tests & quality checks..."
cargo test --all-targets
cargo check --release

# 2. Update Cargo.toml
echo "\n📦 [2/5] Updating version in Cargo.toml..."
sed -i -E "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
cargo check

# 3. Build & Verify binary
echo "\n⚙️  [3/5] Building release binary..."
cargo build --release
COMPILED_VER=$(./target/release/procman --version | awk '{print $2}')

if [ "$COMPILED_VER" != "$NEW_VERSION" ]; then
  echo "❌ Error: Compiled binary version ($COMPILED_VER) does not match target ($NEW_VERSION)!"
  exit 1
fi

# 4. Prompt for CHANGELOG confirmation
echo "\n📝 [4/5] Please ensure CHANGELOG.md has an entry for [v$NEW_VERSION]."
read -p "   Press [Enter] when CHANGELOG.md is ready to commit..." </dev/tty

# 5. Git Commit & Tag
echo "\n🏷️  [5/5] Creating git commit and tag v$NEW_VERSION..."
git add Cargo.toml Cargo.lock CHANGELOG.md README.md docs/
git commit -m "chore(release): v$NEW_VERSION" || true
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

echo "\n🎉 Release v$NEW_VERSION created successfully!"
echo "   To push to remote, run:"
echo "   git push origin main --tags"
