#!/usr/bin/env sh
set -e

REPO="nhatcoi/procman"
INSTALL_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"

echo "==> Installing procman..."

mkdir -p "$INSTALL_DIR"

if command -v cargo >/dev/null 2>&1; then
    echo "==> Building and installing latest procman via cargo..."
    cargo install --git "https://github.com/$REPO.git" --force
else
    echo "==> Error: cargo not found."
    echo "==> Please install Rust & Cargo first: https://rustup.rs"
    echo "==> Or run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo ""
echo "==> procman installed successfully to $INSTALL_DIR/procman"
echo "==> Verify by running:"
echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
echo "    procman --version"
