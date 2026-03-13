#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_DIR"

cargo install --path . --quiet 2>&1
echo "Installed barad-dur to ~/.cargo/bin"
