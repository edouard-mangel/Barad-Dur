#!/bin/sh
# version-bump.test.sh — assert version-bump.sh's bump arithmetic.
#
# The case that matters: while the crate is 0.x, no commit subject may
# push it to 1.0.0. Cargo treats 0.x.y -> 0.(x+1).0 as incompatible, so
# breaking changes are expressible without leaving 0.x, and going 1.0 is
# a product decision rather than a side effect of a "!" in a subject.
#
# Usage: ./scripts/version-bump.test.sh

set -e
SCRIPT="$(cd "$(dirname "$0")" && pwd)/version-bump.sh"
FAILED=0

# Run the script's dry run in a throwaway repo seeded with one commit.
# Echoes the computed next version.
bump_for() {
  start="$1"; subject="$2"
  d=$(mktemp -d)
  (
    cd "$d" || exit 1
    git init -q .
    git config user.email t@example.com
    git config user.name t
    mkdir -p scripts
    cp "$SCRIPT" scripts/version-bump.sh
    chmod +x scripts/version-bump.sh
    printf '[package]\nname = "x"\nversion = "%s"\n' "$start" > Cargo.toml
    git add -A && git commit -qm "chore: init"
    git tag "v$start"
    echo touch > f.txt
    git add -A && git commit -qm "$subject"
    ./scripts/version-bump.sh 2>/dev/null \
      | sed -n 's/^Next version: *//p'
  )
  rm -rf "$d"
}

expect() {
  start="$1"; subject="$2"; want="$3"
  got=$(bump_for "$start" "$subject")
  if [ "$got" = "$want" ]; then
    echo "  ok    $start + '$subject' -> $got"
  else
    echo "  FAIL  $start + '$subject' -> $got (expected $want)"
    FAILED=1
  fi
}

echo "pre-1.0: breaking changes stay in 0.x"
expect 0.20.0 'feat!: breaking thing'        0.21.0
expect 0.20.0 'fix: thing BREAKING CHANGE'   0.21.0
expect 0.21.0 'feat!: breaking thing'        0.22.0

echo "pre-1.0: ordinary bumps unchanged"
expect 0.20.0 'feat: new thing'              0.21.0
expect 0.20.0 'fix: a bug'                   0.20.1

echo "post-1.0: normal semver resumes"
expect 1.2.3  'feat!: breaking thing'        2.0.0
expect 1.2.3  'feat: new thing'              1.3.0
expect 1.2.3  'fix: a bug'                   1.2.4

if [ "$FAILED" -eq 0 ]; then
  echo "version-bump.test.sh: all cases pass"
else
  echo "version-bump.test.sh: FAILURES"
  exit 1
fi
