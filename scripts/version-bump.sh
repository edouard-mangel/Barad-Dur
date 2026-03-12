#!/bin/sh
# version-bump.sh — Deduce the next semver from conventional commits
#
# Reads commits since the last version tag (vX.Y.Z) and determines
# the appropriate bump level:
#   - BREAKING CHANGE or type! → major
#   - feat                     → minor
#   - fix, perf, etc.          → patch
#
# Usage:
#   ./scripts/version-bump.sh          # dry-run: print what would happen
#   ./scripts/version-bump.sh --apply  # update Cargo.toml + create git tag

set -e

CARGO_TOML="Cargo.toml"

# Current version from Cargo.toml
CURRENT=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/')
MAJOR=$(echo "$CURRENT" | cut -d. -f1)
MINOR=$(echo "$CURRENT" | cut -d. -f2)
PATCH=$(echo "$CURRENT" | cut -d. -f3)

# Find the last version tag, or use initial commit if none
LAST_TAG=$(git tag -l 'v*' --sort=-v:refname | head -1)
if [ -z "$LAST_TAG" ]; then
  RANGE="HEAD"
  echo "No version tags found. Scanning all commits."
else
  RANGE="${LAST_TAG}..HEAD"
  echo "Last tag: $LAST_TAG"
fi

# Determine bump level from commit messages
BUMP="none"
while IFS= read -r line; do
  [ -z "$line" ] && continue

  # Check for breaking changes
  if echo "$line" | grep -qE "^[a-z]+(\([a-z0-9_-]+\))?\!:"; then
    BUMP="major"
    break
  fi
  # Check body for BREAKING CHANGE: footer (simplified — checks subject only)
  if echo "$line" | grep -q "BREAKING CHANGE"; then
    BUMP="major"
    break
  fi

  # feat → minor (don't override major)
  if echo "$line" | grep -qE "^feat(\([a-z0-9_-]+\))?:"; then
    if [ "$BUMP" != "major" ]; then
      BUMP="minor"
    fi
  fi

  # fix, perf, refactor, etc. → patch (don't override minor/major)
  if echo "$line" | grep -qE "^(fix|perf|refactor|build|ci|docs|style|test|chore|revert)(\([a-z0-9_-]+\))?:"; then
    if [ "$BUMP" = "none" ]; then
      BUMP="patch"
    fi
  fi
done <<EOF
$(git log $RANGE --pretty=format:"%s")
EOF

if [ "$BUMP" = "none" ]; then
  echo "No conventional commits found since ${LAST_TAG:-initial commit}. Nothing to bump."
  exit 0
fi

# Compute next version
case "$BUMP" in
  major) NEXT="$((MAJOR + 1)).0.0" ;;
  minor) NEXT="${MAJOR}.$((MINOR + 1)).0" ;;
  patch) NEXT="${MAJOR}.${MINOR}.$((PATCH + 1))" ;;
esac

echo "Current version: $CURRENT"
echo "Bump level:      $BUMP"
echo "Next version:    $NEXT"

if [ "$1" = "--apply" ]; then
  # Update Cargo.toml
  sed -i "0,/^version = \"${CURRENT}\"/s//version = \"${NEXT}\"/" "$CARGO_TOML"
  echo ""
  echo "Updated $CARGO_TOML to version $NEXT"

  # Update Cargo.lock
  cargo generate-lockfile --quiet 2>/dev/null || true

  # Create tag
  git add "$CARGO_TOML" Cargo.lock
  git commit -m "chore: bump version to v${NEXT}"
  git tag -a "v${NEXT}" -m "v${NEXT}"
  echo "Created tag v${NEXT}"
  echo ""
  echo "Run 'git push && git push --tags' to publish."
else
  echo ""
  echo "Dry run. Use --apply to update Cargo.toml and create tag."
fi
