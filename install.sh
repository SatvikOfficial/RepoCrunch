#!/bin/bash
set -euo pipefail

REPO="SatvikOfficial/RepoCrunch"
BINARY="repocrunch"
INSTALL_DIR="/usr/local/bin"

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux*)
    ASSET="repocrunch-linux-x86_64"
    ;;
  Darwin*)
    case "$ARCH" in
      arm64|aarch64)
        ASSET="repocrunch-macos-aarch64"
        ;;
      *)
        ASSET="repocrunch-macos-x86_64"
        ;;
    esac
    ;;
  *)
    echo "❌ Unsupported OS: $OS"
    echo "   Use install.ps1 for Windows"
    exit 1
    ;;
esac

echo "🔍 Detecting latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "❌ Could not detect latest release. Install via cargo instead:"
  echo "   cargo install --git https://github.com/$REPO"
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$LATEST/$ASSET"

echo "📦 Downloading RepoCrunch $LATEST ($ASSET)..."
TMPFILE=$(mktemp)
curl -fsSL "$URL" -o "$TMPFILE"
chmod +x "$TMPFILE"

echo "📁 Installing to $INSTALL_DIR/$BINARY..."
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPFILE" "$INSTALL_DIR/$BINARY"
else
  sudo mv "$TMPFILE" "$INSTALL_DIR/$BINARY"
fi

echo ""
echo "✅ RepoCrunch $LATEST installed successfully!"
echo "   Run: repocrunch"
