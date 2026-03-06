#!/usr/bin/env bash
set -euo pipefail

echo "=== Barad-dur Development Environment Setup ==="

# System dependencies (C toolchain, OpenSSL, cmake)
echo "[1/3] Installing system dependencies..."
sudo apt-get update -qq
sudo apt-get install -y -qq build-essential cmake libssl-dev pkg-config

# Rust toolchain
echo "[2/3] Installing Rust toolchain..."
if command -v rustc &>/dev/null; then
    echo "  Rust already installed: $(rustc --version)"
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo "  Installed: $(rustc --version)"
fi

# Build project and download dependencies
echo "[3/3] Building project..."
source "$HOME/.cargo/env"
cargo build

echo ""
echo "=== Setup complete! ==="
echo "  rustc: $(rustc --version)"
echo "  cargo: $(cargo --version)"
echo "  Run 'cargo run -- analyze .' to analyze a git repository."
