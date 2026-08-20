#!/usr/bin/env sh
set -e

REPO="nhatcoi/procman"
INSTALL_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"

echo "==> Installing procman..."

mkdir -p "$INSTALL_DIR"

if command -v cargo >/dev/null 2>&1; then
    TARGET_VERSION="${VERSION:-}"
    if [ -z "$TARGET_VERSION" ]; then
        TARGET_VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)
    fi

    if [ -n "$TARGET_VERSION" ]; then
        echo "==> Building and installing procman $TARGET_VERSION via cargo..."
        cargo install --git "https://github.com/$REPO.git" --tag "$TARGET_VERSION" --force
    else
        echo "==> Building and installing latest procman via cargo..."
        cargo install --git "https://github.com/$REPO.git" --force
    fi
else
    echo "==> Error: cargo not found."
    echo "==> Please install Rust & Cargo first: https://rustup.rs"
    echo "==> Or run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Track anonymous install hit
curl -s "https://hits.sh/github.com/nhatcoi/procman-install.svg" >/dev/null 2>&1 || true

echo ""
echo "==> procman installed successfully to $INSTALL_DIR/procman"
echo "==> Verify by running:"
echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
echo "    procman --version"
