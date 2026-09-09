"""Aggregate cargo-mutants shard logs into a single kill-rate verdict.

Shared by the per-MR gate and the nightly gate so the two cannot drift.
Shards run with `|| true` — they must not fail on a surviving mutant, since
the aggregated rate is the single blocking decision — which means a shard
that genuinely broke also exits 0. Telling those two apart is this script's
main job; the counting is the easy part.

Usage: mutation_gate.py [--threshold N] <log> [<log> ...]
"""

import glob
import re
import sys

# A log with no summary line is only benign when cargo-mutants said why.
NOTHING_TO_DO = re.compile(
    r"Found 0 mutants|[Nn]o mutants found|No mutants to filter"
    r"|Diff changes no Rust source files"
)
SUMMARY = re.compile(r"(\d+) mutants? tested")


class Verdict:
    def __init__(self):
        self.tested = self.caught = self.missed = 0
        self.timeout = self.unviable = 0
        self.all_skipped = True
        self.broken = False
        self.errors = []

    @property
    def viable(self):
        """Unviable mutants do not compile, so no test could kill them."""
        return self.tested - self.unviable

    @property
    def effective_caught(self):
        """TIMEOUTs count as caught: a mutant that pushes the suite past 3x
        its baseline runtime is exercised by tests and cannot ship
        undetected — it just fails slowly."""
        return self.caught + self.timeout

    @property
    def rate(self):
        return self.effective_caught / self.viable * 100 if self.viable else 0.0

    def failed(self, threshold):
        if self.broken:
            return True
        if self.all_skipped or self.viable == 0:
            return False
        return self.rate < threshold

    def render(self, threshold):
        if self.broken:
            return "\n".join(self.errors)
        if self.all_skipped:
            return "All shards skipped — no source changes"
        if self.viable == 0:
            return "No viable mutants — skipping gate"
        head = (
            f"Kill rate: {self.effective_caught}/{self.viable} viable = "
            f"{self.rate:.1f}% ({self.caught} fast + {self.timeout} timeout, "
            f"{self.missed} missed)"
        )
        if self.rate < threshold:
            return (
                f"{head}\nFAIL: {self.rate:.1f}% < {threshold}% required. "
                f"Add tests for {self.missed} missed mutant(s)."
            )
        return f"{head}\nPASS: {self.rate:.1f}% >= {threshold}%"


def aggregate(logs, names=None):
    """Fold shard log *contents* into one verdict."""
    verdict = Verdict()
    names = names or [f"shard {i}" for i in range(len(logs))]

    for name, content in zip(names, logs):
        if "skipping mutation run" in content:
            continue
        if "ERROR cargo test failed" in content:
            verdict.broken = True
            verdict.errors.append(
                f"{name}: baseline tests failed — cannot compute kill rate"
            )
            continue

        match = SUMMARY.search(content)
        if not match:
            # A docs- or CI-only push produces no mutants; that is a skip.
            if NOTHING_TO_DO.search(content):
                continue
            verdict.broken = True
            verdict.errors.append(
                f"{name}: no mutants summary in log — shard run failed "
                "(timed out, most likely)"
            )
            continue

        verdict.all_skipped = False
        # cargo-mutants omits zero categories from the summary line, so parse
        # each one independently within that line and default to 0.
        summary = content[match.start():].splitlines()[0]

        def count(pattern):
            found = re.search(r"(\d+) " + pattern, summary)
            return int(found.group(1)) if found else 0

        verdict.tested += int(match.group(1))
        verdict.caught += count("caught")
        verdict.missed += count("missed")
        verdict.timeout += count("timeouts?")
        verdict.unviable += count("unviable")

    return verdict


def main(argv):
    threshold = 80.0
    args = list(argv)
    if "--threshold" in args:
        i = args.index("--threshold")
        threshold = float(args[i + 1])
        del args[i:i + 2]

    paths = sorted(p for arg in args for p in glob.glob(arg))
    if not paths:
        print("No shard logs found — nothing to aggregate")
        return 1

    contents = [open(p).read() for p in paths]
    verdict = aggregate(contents, names=paths)
    print(verdict.render(threshold))
    return 1 if verdict.failed(threshold) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
