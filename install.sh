#!/bin/sh
# org-gh installer
# Usage: curl -fsSL https://raw.githubusercontent.com/tftio/org-gh/master/install.sh | sh

set -e

REPO="tftio/org-gh"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
ELISP_DIR="${ELISP_DIR:-$HOME/.local/share/org-gh/elisp}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

info() {
    printf "${GREEN}==>${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}warning:${NC} %s\n" "$1"
}

error() {
    printf "${RED}error:${NC} %s\n" "$1" >&2
    exit 1
}

# Check architecture
ARCH=$(uname -m)
OS=$(uname -s)

if [ "$OS" != "Darwin" ]; then
    error "org-gh currently only supports macOS. Found: $OS"
fi

if [ "$ARCH" != "arm64" ]; then
    error "org-gh currently only supports Apple Silicon (aarch64). Found: $ARCH"
fi

# Get latest version
info "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
    error "Could not determine latest version"
fi

info "Latest version: $LATEST"

# Download URL
TARBALL="org-gh-${LATEST}-darwin-aarch64.tar.gz"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/${LATEST}/${TARBALL}"

# Create temp directory
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Download
info "Downloading $TARBALL..."
curl -fsSL "$DOWNLOAD_URL" -o "$TMPDIR/$TARBALL" || error "Download failed"

# Extract
info "Extracting..."
cd "$TMPDIR"
tar -xzf "$TARBALL"

# Install binary
info "Installing binary to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
cp org-gh "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/org-gh"

# Install elisp
info "Installing elisp to $ELISP_DIR..."
mkdir -p "$ELISP_DIR"
cp -r elisp/* "$ELISP_DIR/"

# Check if in PATH
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    warn "$INSTALL_DIR is not in your PATH"
    echo ""
    echo "Add this to your shell profile:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
fi

# Emacs setup instructions
echo ""
info "Installation complete!"
echo ""
echo "To use with Emacs, add to your init.el:"
echo ""
echo "  (add-to-list 'load-path \"$ELISP_DIR\")"
echo "  (require 'org-gh)"
echo ""
echo "Then initialize a file with:"
echo "  M-x org-gh-init RET owner/repo RET"
echo ""
echo "Or from the command line:"
echo "  org-gh init --repo owner/repo file.org"
echo ""
