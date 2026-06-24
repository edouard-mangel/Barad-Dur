#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Barad-dûr — Conference Live Demo Script
#
# Usage:  bash demo.sh
# Press ENTER to advance to the next step.
# Press 'q' then ENTER to quit.
# Press 's' then ENTER to skip a step (shows command, skips execution).
#
# Before going on stage:
#   1. Pre-clone DEMO_REPO so there's no network wait
#   2. Run step 1 once to warm the snapshot cache, then delete it:
#        barad-dur analyze "$DEMO_REPO"
#   3. Set your terminal font to ≥20pt, dark background
#   4. Close notifications / hide the clock
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
TOOL="barad-dur"
DOCKER_IMAGE="lab.frogg.it:5050/edouard_mangel/barad-dur:latest"
DEMO_REPO="${DEMO_REPO:-/tmp/demo-repo}"   # override: DEMO_REPO=/path/to/repo bash demo.sh
SELF_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null || echo ".")"
REPORT_OUT="/tmp/barad-dur-demo-report.html"

# ── Colours ───────────────────────────────────────────────────────────────────
RESET='\033[0m'
BOLD='\033[1m'
DIM='\033[2m'
CYAN='\033[0;36m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
MAGENTA='\033[0;35m'
WHITE='\033[1;37m'
BG_DARK='\033[40m'

_step=0
_total=9

# ── Helpers ───────────────────────────────────────────────────────────────────
hr() {
  printf "${DIM}%s${RESET}\n" "$(printf '─%.0s' $(seq 1 70))"
}

header() {
  clear
  echo
  printf "${BG_DARK}${CYAN}${BOLD}  ▓▓  Barad-dûr  —  The All-Seeing Repository Analyzer  ▓▓${RESET}\n"
  printf "${DIM}  Step %d / %d  │  %s${RESET}\n" "$_step" "$_total" "$1"
  hr
  echo
}

narrate() {
  # Speaker note — shown in yellow so you know what to say
  printf "${YELLOW}  💬  %s${RESET}\n\n" "$*"
}

show_cmd() {
  printf "${WHITE}${BOLD}  \$  ${CYAN}%s${RESET}\n\n" "$*"
}

wait_key() {
  printf "${DIM}  ↵  Press ENTER to run  │  s = skip  │  q = quit${RESET}  "
  read -r _input
  case "${_input,,}" in
    q) echo; echo "  Demo ended."; exit 0 ;;
    s) return 1 ;;
    *) return 0 ;;
  esac
}

run_step() {
  local title="$1"; shift
  local note="$1";  shift
  local cmd="$*"

  _step=$(( _step + 1 ))
  header "$title"
  narrate "$note"
  show_cmd "$cmd"

  if wait_key; then
    echo
    hr
    eval "$cmd"
    hr
    echo
    printf "${DIM}  ↵  Press ENTER for next step…${RESET}  "
    read -r _
  fi
}

# ══════════════════════════════════════════════════════════════════════════════
# INTRO SLIDE
# ══════════════════════════════════════════════════════════════════════════════
clear
echo
printf "${CYAN}${BOLD}"
cat << 'BANNER'
  ██████╗  █████╗ ██████╗  █████╗ ██████╗       ██████╗ ██╗   ██╗██████╗
  ██╔══██╗██╔══██╗██╔══██╗██╔══██╗██╔══██╗      ██╔══██╗██║   ██║██╔══██╗
  ██████╔╝███████║██████╔╝███████║██║  ██║      ██║  ██║██║   ██║██████╔╝
  ██╔══██╗██╔══██║██╔══██╗██╔══██║██║  ██║      ██║  ██║██║   ██║██╔══██╗
  ██████╔╝██║  ██║██║  ██║██║  ██║██████╔╝      ██████╔╝╚██████╔╝██║  ██║
  ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝       ╚═════╝  ╚═════╝ ╚═╝  ╚═╝
BANNER
printf "${RESET}"
echo
printf "${WHITE}          The All-Seeing Repository Analyzer${RESET}\n"
printf "${DIM}          Live Demo  —  Press ENTER to begin${RESET}\n"
echo
read -r _

# ══════════════════════════════════════════════════════════════════════════════
# STEP 1 — Zero-install: Docker one-liner on a real repo
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "One command. No install." \
  "No setup. No config files. Mount any repo and get a score in seconds." \
  "docker run --rm -v ${DEMO_REPO}:/repo ${DOCKER_IMAGE}"

# ══════════════════════════════════════════════════════════════════════════════
# STEP 2 — Time window: last 3 months vs default
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "Scope to recent activity" \
  "The default looks back 6 months. But what's the codebase been doing lately?" \
  "${TOOL} analyze ${DEMO_REPO} --since 3months"

# ══════════════════════════════════════════════════════════════════════════════
# STEP 3 — Full history
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "Or see the full history" \
  "No time limit. Every commit, every author, from day one." \
  "${TOOL} analyze ${DEMO_REPO} --all"

# ══════════════════════════════════════════════════════════════════════════════
# STEP 4 — Coupling: the surprise metric
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "Logical coupling — the hidden dependency graph" \
  "These two files always change together. Nobody planned that. The code doesn't show it. The git log does." \
  "${TOOL} analyze ${DEMO_REPO} --coupling"

# ══════════════════════════════════════════════════════════════════════════════
# STEP 5 — HTML report
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "Full interactive report" \
  "Everything in a single self-contained HTML file. Send it by email, open it offline, no server needed." \
  "${TOOL} analyze ${DEMO_REPO} --html -o ${REPORT_OUT} && echo 'Report: ${REPORT_OUT}'"

_step=$(( _step - 1 ))  # open browser doesn't count as a numbered step
header "Open the HTML report"
narrate "Let's open it in the browser."
show_cmd "xdg-open ${REPORT_OUT}   # or: open ${REPORT_OUT} on macOS"
if wait_key; then
  xdg-open "${REPORT_OUT}" 2>/dev/null || open "${REPORT_OUT}" 2>/dev/null || \
    echo "  Open ${REPORT_OUT} in your browser manually."
  echo
  printf "${DIM}  ↵  Press ENTER when done…${RESET}  "
  read -r _
fi
_step=$(( _step + 1 ))

# ══════════════════════════════════════════════════════════════════════════════
# STEP 6 — JSON output for tooling
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "Machine-readable output for your own tooling" \
  "Pipe it into jq, feed it to a dashboard, store it in your data warehouse." \
  "${TOOL} analyze ${DEMO_REPO} --json | jq '{score: .score, band: .band, top_actions: [.actions[:3][].title]}'"

# ══════════════════════════════════════════════════════════════════════════════
# STEP 7 — CI gate
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "CI quality gate — exit 1 below the threshold" \
  "Drop this in your pipeline. If the score falls below 70, the build fails. Non-negotiable." \
  "${TOOL} gate ${DEMO_REPO} --min-score 70; echo \"Exit code: \$?\""

# ══════════════════════════════════════════════════════════════════════════════
# STEP 8 — Dogfood: run on Barad-dûr itself
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "Dogfooding — analyzing the analyzer" \
  "We run Barad-dûr on its own repo in CI on every push. Here's what it thinks of itself right now." \
  "${TOOL} analyze ${SELF_REPO}"

# ══════════════════════════════════════════════════════════════════════════════
# STEP 9 — Contributors (bonus if time allows)
# ══════════════════════════════════════════════════════════════════════════════
run_step \
  "Team health — bus factor and ownership" \
  "Who owns what? What happens if that person leaves? The contributors view surfaces concentration risk." \
  "${TOOL} contributors ${DEMO_REPO}"

# ══════════════════════════════════════════════════════════════════════════════
# OUTRO
# ══════════════════════════════════════════════════════════════════════════════
clear
echo
hr
printf "${CYAN}${BOLD}  Try it now:${RESET}\n\n"
printf "  ${WHITE}docker run --rm -v ./your-repo:/repo %s${RESET}\n\n" "${DOCKER_IMAGE}"
printf "${DIM}  Source & issues:${RESET}  ${WHITE}github.com/edouard-mangel/Barad-Dur${RESET}\n"
printf "${DIM}  Registry:${RESET}         ${WHITE}lab.frogg.it:5050/edouard_mangel/barad-dur${RESET}\n"
hr
echo
printf "${DIM}  Demo script finished. Thank you!${RESET}\n"
echo
