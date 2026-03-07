#!/usr/bin/env bash
set -euo pipefail

echo "=== Barad-dur Development Environment Setup ==="

# System dependencies (C toolchain, OpenSSL, cmake)
echo "[1/3] Installing system dependencies..."
sudo apt-get update -qq
sudo apt-get install -y -qq build-essential cmake libssl-dev pkg-config

# Rust toolchain
echo "[2/4] Installing Rust toolchain..."
if command -v rustc &>/dev/null; then
    echo "  Rust already installed: $(rustc --version)"
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo "  Installed: $(rustc --version)"
fi

source "$HOME/.cargo/env"

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
echo "  Run 'cargo run -- analyze .' to analyze a git repository."
echo ""
echo "NOTE: To use cargo in your current shell, run:"
echo "  source \"\$HOME/.cargo/env\""
