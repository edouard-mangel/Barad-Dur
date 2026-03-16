#!/usr/bin/env bash
set -euo pipefail

echo "=== Barad-dur Development Environment Setup ==="

# System dependencies (C toolchain, OpenSSL, cmake)
echo "[1/4] Checking system dependencies..."
MISSING_DEPS=()
command -v gcc &>/dev/null || MISSING_DEPS+=(build-essential)
command -v cmake &>/dev/null || MISSING_DEPS+=(cmake)
command -v pkg-config &>/dev/null || MISSING_DEPS+=(pkg-config)
# libssl-dev has no binary; check for the pkg-config file
if ! pkg-config --exists openssl 2>/dev/null && ! [ -f /usr/include/openssl/ssl.h ]; then
    MISSING_DEPS+=(libssl-dev)
fi

if [ ${#MISSING_DEPS[@]} -gt 0 ]; then
    echo "  Missing: ${MISSING_DEPS[*]}"
    echo "  Installing via apt (requires sudo)..."
    sudo apt-get update -qq
    sudo apt-get install -y -qq "${MISSING_DEPS[@]}"
else
    echo "  All system dependencies already installed."
fi

# Rust toolchain
echo "[2/4] Installing Rust toolchain..."
if command -v rustc &>/dev/null; then
    echo "  Rust already installed: $(rustc --version)"
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    echo "  Installed Rust."
fi

# Ensure cargo is on PATH for the rest of the script
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

# Rust tools (rustfmt, clippy, cargo-watch)
echo "[3/4] Installing Rust tools..."
rustup component add rustfmt clippy
if ! command -v cargo-watch &>/dev/null; then
    cargo install cargo-watch
else
    echo "  cargo-watch already installed"
fi

# Build and install the binary to ~/.cargo/bin
echo "[4/4] Installing barad-dur..."
cargo install --path .

echo ""
echo "=== Setup complete! ==="
echo "  rustc:        $(rustc --version)"
echo "  cargo:        $(cargo --version)"
echo "  rustfmt:      $(rustfmt --version)"
echo "  clippy:       $(cargo clippy --version)"
echo "  cargo-watch:  $(cargo watch --version)"
echo ""
echo "  Run 'barad-dur analyze .' to analyze a git repository."
echo ""

# Ensure cargo is on PATH for future shells
SHELL_RC=""
if [ -n "${ZSH_VERSION:-}" ] || [[ "${SHELL:-}" == */zsh ]]; then
    SHELL_RC="$HOME/.zshrc"
elif [ -n "${BASH_VERSION:-}" ] || [[ "${SHELL:-}" == */bash ]]; then
    SHELL_RC="$HOME/.bashrc"
fi

if [ -n "$SHELL_RC" ] && [ -f "$SHELL_RC" ] && ! grep -q 'cargo/env' "$SHELL_RC" 2>/dev/null; then
    echo '. "$HOME/.cargo/env"' >> "$SHELL_RC"
    echo "Added cargo to PATH in $SHELL_RC"
fi

echo "NOTE: To use barad-dur in your current shell, run:"
echo "  source \"\$HOME/.cargo/env\""
