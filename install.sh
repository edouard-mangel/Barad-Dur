#!/usr/bin/env bash
set -euo pipefail

# ── Barad-dûr installer ─────────────────────────────────────────────
# Installs the CLI locally or builds a Docker container.
#
# Usage:
#   ./install.sh              local install (cargo + dashboard + hooks)
#   ./install.sh --docker     build Docker image "barad-dur"
#   ./install.sh --docker -t myorg/barad-dur:latest

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ── helpers ──────────────────────────────────────────────────────────

info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*"; }
ok()    { printf '\033[1;32m[ok]\033[0m    %s\n' "$*"; }
warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$*"; }
die()   { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

require() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not found. Please install it first."
}

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --docker        Build a Docker container instead of installing locally
  -t, --tag TAG   Docker image tag (default: barad-dur:latest)
  -h, --help      Show this help message
EOF
    exit 0
}

# ── parse flags ──────────────────────────────────────────────────────

MODE="local"
DOCKER_TAG="barad-dur:latest"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --docker)   MODE="docker"; shift ;;
        -t|--tag)   DOCKER_TAG="${2:?'--tag requires a value'}"; shift 2 ;;
        -h|--help)  usage ;;
        *)          die "Unknown option: $1 (see --help)" ;;
    esac
done

# ── docker path ──────────────────────────────────────────────────────

install_docker() {
    require docker

    info "Building Docker image '${DOCKER_TAG}'…"
    docker build -t "$DOCKER_TAG" "$SCRIPT_DIR"
    ok "Docker image built: ${DOCKER_TAG}"

    echo ""
    ok "Installation complete!"
    info "Run the container with:"
    info "  docker run --rm -v \$(pwd):/repo:ro ${DOCKER_TAG}"
}

# ── local path ───────────────────────────────────────────────────────

install_local() {
    # ── pre-flight checks ────────────────────────────────────────────
    info "Checking prerequisites…"

    require cargo
    require rustc

    RUST_VERSION="$(rustc --version | awk '{print $2}')"
    info "Rust ${RUST_VERSION} detected"

    # pnpm is optional — only needed for the dashboard
    HAS_PNPM=false
    if command -v pnpm >/dev/null 2>&1; then
        HAS_PNPM=true
    fi

    # ── build & install CLI ──────────────────────────────────────────
    info "Building and installing barad-dur CLI (release mode)…"
    cargo install --path "$SCRIPT_DIR"
    ok "barad-dur installed to $(command -v barad-dur 2>/dev/null || echo '~/.cargo/bin/barad-dur')"

    # ── dashboard dependencies ───────────────────────────────────────
    if [ -d "$SCRIPT_DIR/dashboard" ]; then
        if $HAS_PNPM; then
            info "Installing dashboard dependencies…"
            (cd "$SCRIPT_DIR/dashboard" && pnpm install)
            ok "Dashboard ready — run 'make dashboard' to start the dev server"
        else
            warn "pnpm not found — skipping dashboard setup."
            warn "Install pnpm (https://pnpm.io) then run: cd dashboard && pnpm install"
        fi
    fi

    # ── git hooks ────────────────────────────────────────────────────
    if [ -d "$SCRIPT_DIR/hooks" ]; then
        info "Configuring git hooks…"
        git -C "$SCRIPT_DIR" config core.hooksPath hooks
        ok "Git hooks configured (commit-msg + pre-push)"
    fi

    echo ""
    ok "Installation complete!"
    info "Run 'barad-dur --help' to get started."
}

# ── dispatch ─────────────────────────────────────────────────────────

case "$MODE" in
    docker) install_docker ;;
    local)  install_local  ;;
esac
