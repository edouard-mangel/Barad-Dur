#!/bin/sh
# mutation-campaign.test.sh — assert mutation-campaign.sh's bookkeeping.
#
# The campaign runs for hours, so what it must never do is lose or mix work
# silently: skip a file, reuse results computed under another base or test
# filter, fail only at the end, or hand cargo-mutants an unreadable diff.
# A fake `cargo` records each call and, like cargo-mutants, only reads
# standard `+++ b/<path>` diff headers.
#
# Usage: ./scripts/mutation-campaign.test.sh

set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
FAILED=0

ok() { echo "  ok    $1"; }
ko() { echo "  FAIL  $1"; FAILED=1; }

# A throwaway repo whose `main` branch is the base and HEAD changes two files
# whose paths only differ by a separator, plus a fake cargo on PATH.
setup() {
  d=$(mktemp -d)
  mkdir -p "$d/repo/scripts" "$d/repo/src/a" "$d/bin"
  cp "$HERE/mutation-campaign.sh" "$HERE/mutation_gate.py" "$d/repo/scripts/"
  cat > "$d/bin/cargo" <<'EOF'
#!/bin/sh
case " $* " in *" --version "*) exit 0 ;; esac
case " $* " in
  *" --list "*)
    for a in "$@"; do [ "$prev" = "--in-diff" ] && diff="$a"; prev="$a"; done
    sed -n 's|^+++ b/\(.*\.rs\)$|\1:1:1: replace body|p' "$diff"
    exit 0 ;;
esac
for a in "$@"; do
  [ "$prev" = "--file" ] && file="$a"
  [ "$prev" = "--output" ] && out="$a"
  prev="$a"
done
# One bracket per argument: "$*" would join them with spaces, which is exactly
# the difference a word-splitting bug makes, so the record would not show it.
{ printf '%s|' "$file"; for a in "$@"; do printf '[%s]' "$a"; done; printf '\n'; } >> "$RECORD"
mkdir -p "$out/mutants.out"
echo "1 mutant tested in 1s: 1 caught"
exit 0
EOF
  chmod +x "$d/bin/cargo"
  (
    cd "$d/repo"
    git init -q -b main .
    git config user.email t@example.com
    git config user.name t
    echo 'fn a() {}' > src/a/b.rs
    echo 'fn b() {}' > src/a_b.rs
    git add -A && git commit -qm init
    git checkout -qb change
    echo 'fn a() { 1; }' > src/a/b.rs
    echo 'fn b() { 2; }' > src/a_b.rs
    git commit -qam change
  )
  export RECORD="$d/record"
  : > "$RECORD"
}

campaign() {
  (cd "$d/repo" && PATH="$d/bin:$PATH" BASE_REF=main MUTATION_OUT="$d/out" \
    sh scripts/mutation-campaign.sh) > "$d/log" 2>&1
}

echo "every file runs, even when paths only differ by a separator"
setup
campaign || true
runs=$(cut -d'|' -f1 "$RECORD" | sort | tr '\n' ' ')
[ "$runs" = "src/a/b.rs src/a_b.rs " ] && ok "$runs" || ko "runs: '$runs'"

echo "no test filter unless MUTATION_TESTS asks for one"
grep -q -- '--cargo-test-arg=-E' "$RECORD" && ko "a default filter was passed" || ok "no -E filter"
rm -rf "$d"

echo "a filter with spaces reaches nextest as one argument"
setup
(cd "$d/repo" && PATH="$d/bin:$PATH" BASE_REF=main MUTATION_OUT="$d/out" \
  MUTATION_TESTS='test(a) | test(b)' sh scripts/mutation-campaign.sh) > "$d/log" 2>&1 || true
grep -qF -- '[--cargo-test-arg=-E][--cargo-test-arg=test(a) | test(b)]' "$RECORD" &&
  [ "$(grep -o -- '--cargo-test-arg' "$RECORD" | wc -l)" -eq 4 ] &&
  ok "filter passed whole" || ko "filter args: $(cat "$RECORD")"
rm -rf "$d"

echo "a resume under another test filter is refused"
setup
campaign || true
if (cd "$d/repo" && PATH="$d/bin:$PATH" BASE_REF=main MUTATION_OUT="$d/out" \
  MUTATION_TESTS='test(other)' sh scripts/mutation-campaign.sh) > "$d/log" 2>&1; then
  ko "resume with another filter succeeded"
else
  grep -q 'MUTATION_TESTS' "$d/log" && ok "refused: $(tail -1 "$d/log")" || ko "refused without naming the cause: $(cat "$d/log")"
fi
rm -rf "$d"

echo "a resume at another HEAD is refused"
# The output directory is keyed by HEAD only when MUTATION_OUT is left alone,
# and the refusal message recommends setting it. Under a fixed directory the
# base and filter can both still match after new commits, and every file is
# already DONE, so the campaign would reprint the previous revision's verdict.
setup
campaign || true
(cd "$d/repo" && echo 'fn c() { 3; }' > src/c.rs && git add -A && git commit -qm more)
if (cd "$d/repo" && PATH="$d/bin:$PATH" BASE_REF=main MUTATION_OUT="$d/out" \
  sh scripts/mutation-campaign.sh) > "$d/log" 2>&1; then
  ko "resume at another HEAD succeeded: $(cat "$d/log")"
else
  grep -q 'HEAD' "$d/log" && ok "refused: $(tail -1 "$d/log")" || ko "refused without naming the cause: $(cat "$d/log")"
fi
rm -rf "$d"

echo "a missing python3 stops the campaign before anything is written"
setup
mkdir "$d/nopy"
for tool in sh git cargo cut sort uniq awk mkdir mv rm touch date tr cat sed dirname grep; do
  path=$(PATH="$d/bin:$PATH" command -v "$tool") && ln -s "$path" "$d/nopy/$tool"
done
if (cd "$d/repo" && PATH="$d/nopy" BASE_REF=main MUTATION_OUT="$d/out" \
  "$d/nopy/sh" scripts/mutation-campaign.sh) > "$d/log" 2>&1; then
  ko "ran without python3"
elif [ -e "$d/out" ] || [ -s "$RECORD" ]; then
  ko "wrote output before failing: $(cat "$d/log")"
else
  grep -q python3 "$d/log" && ok "refused: $(cat "$d/log")" || ko "unclear failure: $(cat "$d/log")"
fi
rm -rf "$d"

echo "the user's diff configuration does not change the diff handed to cargo-mutants"
setup
(cd "$d/repo" && git config color.ui always && git config diff.noprefix true)
campaign || true
if grep -q "$(printf '\033')" "$d/out/change.diff"; then
  ko "change.diff holds colour codes"
elif [ "$(wc -l < "$RECORD")" -eq 2 ]; then
  ok "both files listed"
else
  ko "files listed: $(cat "$RECORD"; cat "$d/log")"
fi
rm -rf "$d"

exit "$FAILED"
