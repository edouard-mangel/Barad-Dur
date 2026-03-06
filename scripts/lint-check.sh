#!/usr/bin/env bash
# PostToolUse hook: runs cargo fmt --check and clippy after Edit/Write on Rust files.
# Only runs when the edited file is a .rs or .toml file inside myTool/.

set -euo pipefail

TOOL_INPUT="${TOOL_INPUT:-}"
PROJECT_DIR="/home/edouard/WS/tool/myTool"

# Only trigger for files in the project
if ! echo "$TOOL_INPUT" | grep -q "$PROJECT_DIR"; then
    exit 0
fi

# Only trigger for Rust-related files
if ! echo "$TOOL_INPUT" | grep -qE '\.(rs|toml)'; then
    exit 0
fi

export PATH="$HOME/.cargo/bin:$PATH"
cd "$PROJECT_DIR"

# Run fmt check
if ! cargo fmt -- --check >/dev/null 2>&1; then
    echo "⚠️  cargo fmt check failed — run 'cargo fmt' to fix formatting"
    exit 1
fi

# Run clippy
if ! cargo clippy --all-targets -- -D warnings 2>&1 | grep -q "^error"; then
    exit 0
else
    echo "⚠️  clippy found warnings — fix them before proceeding"
    exit 1
fi
