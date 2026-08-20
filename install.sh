#!/usr/bin/env sh
set -e

REPO="nhatcoi/procman"

# Determine OS
OS="$(uname -s)"
case "$OS" in
    Linux*)     OS_TYPE="unknown-linux-gnu";;
    Darwin*)    OS_TYPE="apple-darwin";;
    *)          OS_TYPE="";;
esac

# Determine Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)   ARCH_TYPE="x86_64";;
    aarch64|arm64)  ARCH_TYPE="aarch64";;
    *)              ARCH_TYPE="";;
esac

# Determine installation directory (prefer ~/.cargo/bin if exists, else ~/.local/bin)
if [ -n "$CARGO_HOME" ] && [ -d "$CARGO_HOME/bin" ]; then
    INSTALL_DIR="$CARGO_HOME/bin"
elif [ -d "$HOME/.cargo/bin" ]; then
    INSTALL_DIR="$HOME/.cargo/bin"
elif [ -d "$HOME/.local/bin" ]; then
    INSTALL_DIR="$HOME/.local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
fi
mkdir -p "$INSTALL_DIR"

echo "==> Installing procman..."

# Determine target version
TARGET_VERSION="${VERSION:-}"
if [ -z "$TARGET_VERSION" ]; then
    TARGET_VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)
fi

INSTALLED=0

# 1. Try pre-compiled binary first (Instant 2-second install without Rust/Cargo)
if [ -n "$OS_TYPE" ] && [ -n "$ARCH_TYPE" ] && [ -n "$TARGET_VERSION" ]; then
    TARGET_TRIPLE="${ARCH_TYPE}-${OS_TYPE}"
    TAR_NAME="procman-${TARGET_TRIPLE}.tar.gz"
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TARGET_VERSION/$TAR_NAME"
    
    echo "==> Detected system: $TARGET_TRIPLE"
    echo "==> Downloading pre-compiled binary ($TARGET_VERSION)..."
    
    TMP_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t 'procman')
    if curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$TAR_NAME" 2>/dev/null; then
        tar -xzf "$TMP_DIR/$TAR_NAME" -C "$TMP_DIR"
        chmod +x "$TMP_DIR/procman"
        mv "$TMP_DIR/procman" "$INSTALL_DIR/procman"
        rm -rf "$TMP_DIR"
        INSTALLED=1
    else
        rm -rf "$TMP_DIR"
        echo "==> Pre-compiled binary not found for $TARGET_TRIPLE ($TARGET_VERSION). Falling back to cargo build..."
    fi
fi

# 2. Fallback to cargo install if pre-compiled binary is not available
if [ "$INSTALLED" -eq 0 ]; then
    if command -v cargo >/dev/null 2>&1; then
        if [ -n "$TARGET_VERSION" ]; then
            echo "==> Building and installing procman $TARGET_VERSION via cargo..."
            cargo install --git "https://github.com/$REPO.git" --tag "$TARGET_VERSION" --force
        else
            echo "==> Building and installing latest procman via cargo..."
            cargo install --git "https://github.com/$REPO.git" --force
        fi
        INSTALLED=1
    else
        echo "==> Error: Could not download pre-compiled binary and 'cargo' was not found on this system."
        echo "==> Please install Rust & Cargo: https://rustup.rs"
        exit 1
    fi
fi

echo ""
echo "==> procman installed successfully to $INSTALL_DIR/procman"
echo "==> Verify by running:"
echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
echo "    procman --version"
