#!/bin/sh
# Mutation campaign for a change too large for the per-MR CI gate.
#
# The per-MR shards fit roughly 100-150 mutants in their 4 h budget; a large
# feature branch can produce close to a thousand. This runs the same in-diff
# mutants locally, one cargo-mutants run per source file with its own output
# directory, so an interrupted campaign resumes where it stopped and never
# overwrites finished results. The verdict uses scripts/mutation_gate.py, the
# policy the CI gate applies (timeouts count as caught, unviable excluded, 80%).
#
# Usage (from anywhere inside the checkout, on the commit to evaluate):
#   scripts/mutation-campaign.sh
# Re-run the same command to resume. Environment overrides:
#   BASE_REF        diff base                          (default: origin/main)
#   MUTATION_OUT    output root                        (default: target/mutation-campaign/<head>)
#   MUTATION_JOBS   parallel cargo-mutants jobs        (default: 2)
#   CARGO_BUILD_JOBS rustc parallelism per build       (default: 8)
#   MUTATION_TESTS  nextest filter expression          (default: none, every library test)
#
# Only library tests run (and only those MUTATION_TESTS selects, when set), so
# the kill rate is a lower bound: a surviving mutant may still be killed by an
# integration test. A resume must use the same base and filter as the run it
# continues; the script refuses to mix results computed under different ones.
set -u

REPO=$(command git rev-parse --show-toplevel) || exit 1
cd "$REPO" || exit 1

BASE_REF=${BASE_REF:-origin/main}
HEAD_SHA=$(command git rev-parse --short=10 HEAD)
OUT=${MUTATION_OUT:-$REPO/target/mutation-campaign/$HEAD_SHA}
JOBS=${MUTATION_JOBS:-2}
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-8}"
TESTS=${MUTATION_TESTS:-}
DIFF="$OUT/change.diff"
SETTINGS="$OUT/settings"

fail() {
  echo "$1" >&2
  exit 1
}

# Both cargo subcommands must exist before anything is written: a missing tool
# otherwise surfaces much later, or disguised as "no mutants".
cargo mutants --version > /dev/null 2>&1 ||
  fail "cargo-mutants is missing: cargo binstall cargo-mutants (or cargo install --locked cargo-mutants)"
cargo nextest --version > /dev/null 2>&1 ||
  fail "cargo-nextest is missing: cargo binstall cargo-nextest (or cargo install --locked cargo-nextest)"
command -v python3 > /dev/null ||
  fail "python3 is missing: it computes the verdict (scripts/mutation_gate.py)."
command git rev-parse --verify --quiet "$BASE_REF" > /dev/null ||
  fail "Base ref $BASE_REF not found: run git fetch origin, or set BASE_REF."
MERGE_BASE=$(command git merge-base "$BASE_REF" HEAD) ||
  fail "No merge base between $BASE_REF and HEAD."

if [ -n "$(command git status --porcelain --untracked-files=no)" ]; then
  fail "Tracked files have uncommitted changes: commit or stash them first."
fi

# Results of a resumed campaign are only comparable under the same commit, base
# and filter. HEAD belongs in here even though the default output directory is
# named after it: MUTATION_OUT overrides that name, and the refusal below is
# what recommends MUTATION_OUT, so without this the recommended path is the one
# that silently reprints an older revision's verdict.
WANTED=$(printf 'merge-base %s\nhead %s\ntests %s\n' \
  "$MERGE_BASE" "$(command git rev-parse HEAD)" "$TESTS")
if [ -f "$SETTINGS" ] && [ "$(cat "$SETTINGS")" != "$WANTED" ]; then
  fail "$OUT was started at another HEAD, base or MUTATION_TESTS filter ($(tr '\n' ' ' < "$SETTINGS")); resume it from that commit, or set MUTATION_OUT."
fi

mkdir -p "$OUT"
printf '%s\n' "$WANTED" > "$SETTINGS"
if [ ! -s "$DIFF" ]; then
  # Write through a temporary file so a failed diff never leaves an empty one behind.
  # The format is pinned: cargo-mutants needs uncoloured a/ b/ headers whatever
  # the user's git configuration says.
  command git -c color.ui=never diff --no-color --no-ext-diff --src-prefix=a/ --dst-prefix=b/ \
    "$MERGE_BASE" HEAD > "$DIFF.tmp" && [ -s "$DIFF.tmp" ] ||
    fail "Empty or failed diff between $BASE_REF and HEAD."
  mv "$DIFF.tmp" "$DIFF"
fi
echo "head $HEAD_SHA, base $BASE_REF, output $OUT"

# One run per file that has in-diff mutants, smallest first so results arrive early.
cargo mutants --list --in-diff "$DIFF" > "$OUT/mutants.list" ||
  fail "cargo mutants --list failed (see the output above)."
FILES=$(cut -d: -f1 "$OUT/mutants.list" | sort | uniq -c | sort -n | awk '{print $2}')
[ -n "$FILES" ] || fail "No in-diff mutants."

for FILE in $FILES; do
  # Escape `_` first so the directory name maps back to exactly one path.
  DIR="$OUT/$(echo "$FILE" | sed 's/_/_u/g; s|/|_s|g; s/\./_d/g')"
  if [ -f "$DIR/DONE" ]; then
    echo "skip   $FILE"
    continue
  fi
  rm -rf "$DIR"
  mkdir -p "$DIR"
  echo "start  $FILE  $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  NEXTEST_PROFILE=mutation BARAD_DUR_TEST_REPO="$REPO" cargo mutants \
    --in-diff "$DIFF" --file "$FILE" \
    --test-tool nextest --cargo-arg=--lib ${TESTS:+--cargo-test-arg=-E "--cargo-test-arg=$TESTS"} \
    --timeout-multiplier 3 --minimum-test-timeout 120 --jobs "$JOBS" --gitignore true \
    --output "$DIR" > "$DIR/run.log" 2>&1
  RC=$?
  # cargo-mutants exit codes: 0 all caught, 2 some missed, 3 some timed out.
  case "$RC" in
    0|2|3) touch "$DIR/DONE"; echo "done   $FILE rc=$RC  $(date -u +%Y-%m-%dT%H:%M:%SZ)" ;;
    *) echo "FAILED $FILE rc=$RC, see $DIR/run.log" >&2; exit "$RC" ;;
  esac
done

echo
echo "Survivors to review:"
cat "$OUT"/*/mutants.out/missed.txt 2>/dev/null
echo
python3 scripts/mutation_gate.py "$OUT/*/run.log"
