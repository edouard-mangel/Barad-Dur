"""Aggregate cargo-mutants shard logs into a single kill-rate verdict.

Shared by the per-MR gate and the nightly gate so the two cannot drift.
Shards run with `|| true` — they must not fail on a surviving mutant, since
the aggregated rate is the single blocking decision — which means a shard
that genuinely broke also exits 0. Telling those two apart is this script's
main job; the counting is the easy part.

Usage: mutation_gate.py [--threshold N] [--require-override] <log> [<log> ...]

--require-override applies on a merge request: when the shards skipped the run
because the diff carries more mutants than they can test, the verdict then
requires a "Mutation gate override" section in MUTATION_OVERRIDE_TEXT (the
merge request description).
"""

import glob
import os
import re
import sys

# A log with no summary line is only benign when cargo-mutants said why.
NOTHING_TO_DO = re.compile(
    r"Found 0 mutants|[Nn]o mutants found|No mutants to filter"
    r"|Diff changes no Rust source files"
)
SUMMARY = re.compile(r"(\d+) mutants? tested")
# A shard writes this instead of running when the MR carries more in-diff
# mutants than the shards can test inside their wall clock.
OVER_CAPACITY = re.compile(r"MUTATION OVER CAPACITY: (\d+) in-diff mutants exceed")
# The section the author adds to the MR description to carry local evidence.
# Case and heading style vary between authors and cost a pipeline when refused,
# so both an ATX heading and a bold line count.
OVERRIDE_MARKER = re.compile(r"^ {0,3}(?:#+|\*\*) *Mutation gate override", re.M | re.I)
# A fenced block quotes the mechanism (this file's own FAIL message, say); it
# does not attest to a campaign, so it is removed before the section is read.
FENCE = re.compile(r"^```.*?^```", re.M | re.S)
# What the section has to carry: a kill rate, and the commit it was measured
# at. Without them the gate enforces a heading, and a heading is not evidence.
OVERRIDE_RATE = re.compile(r"kill rate[^0-9]{0,20}(\d+(?:\.\d+)?) ?%", re.I)
OVERRIDE_COMMIT = re.compile(r"\bat ([0-9a-f]{7,40})\b")


class Verdict:
    def __init__(self):
        self.tested = self.caught = self.missed = 0
        self.timeout = self.unviable = 0
        self.all_skipped = True
        self.broken = False
        self.errors = []
        # Shards that carried a summary line, for the messages that have to
        # say how much of the diff the verdict actually covers.
        self.reported = []
        self.over_capacity = 0
        # Set by main(): only a merge request can carry an override section.
        self.require_override = False
        self.override_text = ""
        # GitLab keeps only the first 2700 characters of a description in
        # CI_MERGE_REQUEST_DESCRIPTION. A section written past that point is
        # invisible here, so the absence of the marker has two causes and the
        # verdict has to name the right one.
        self.override_truncated = False

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

    @property
    def override_claim(self):
        """The measurement the override section carries, as (rate, commit), or
        None when it carries none — which is the case a heading alone makes."""
        text = FENCE.sub("", self.override_text)
        marker = OVERRIDE_MARKER.search(text)
        if not marker:
            return None
        section = text[marker.end():]
        rate = OVERRIDE_RATE.search(section)
        commit = OVERRIDE_COMMIT.search(section)
        if not rate or not commit:
            return None
        return float(rate.group(1)), commit.group(1)

    def failed(self, threshold):
        if self.broken:
            return True
        if self.over_capacity and self.all_skipped:
            if not self.require_override:
                return False
            claim = self.override_claim
            return claim is None or claim[0] < threshold
        if self.all_skipped or self.viable == 0:
            return False
        return self.rate < threshold

    def render(self, threshold):
        if self.broken:
            return "\n".join(self.errors)
        if self.over_capacity and self.all_skipped:
            head = (
                f"Over capacity: {self.over_capacity} in-diff mutants, more than "
                "the shards can test in their wall clock"
            )
            if not self.require_override:
                return f"{head}\nPASS: the nightly sweep covers this branch"
            claim = self.override_claim
            if claim and claim[0] >= threshold:
                return (
                    f"{head}\nPASS: the description documents a local campaign at "
                    f"{claim[1]} with a kill rate of {claim[0]:.1f}%"
                )
            if claim:
                return (
                    f"{head}\nFAIL: the documented campaign at {claim[1]} killed "
                    f"{claim[0]:.1f}%, below the {threshold:.0f}% this gate requires."
                )
            if self.override_truncated:
                return (
                    f"{head}\nFAIL: the merge request description is truncated at "
                    "2700 characters, so the gate cannot see a "
                    '"Mutation gate override" section written below that point. '
                    "Move it above, or shorten what precedes it."
                )
            return (
                f"{head}\nFAIL: run scripts/mutation-campaign.sh and add a "
                '"Mutation gate override" section to the description stating '
                '"kill rate N% at <commit>" and the disposition of every '
                "survivor."
            )
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
        over = OVER_CAPACITY.search(content)
        if over:
            verdict.over_capacity = max(verdict.over_capacity, int(over.group(1)))
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

        verdict.reported.append(name)
        verdict.tested += int(match.group(1))
        verdict.caught += count("caught")
        verdict.missed += count("missed")
        verdict.timeout += count("timeouts?")
        verdict.unviable += count("unviable")

    # Every shard reads the same diff, so they go over capacity together or
    # not at all. A mix means some were re-run under a different capacity, and
    # a rate computed from the ones that reported would cover a fraction of
    # the diff while rendering like an ordinary verdict.
    if verdict.over_capacity and not verdict.all_skipped:
        verdict.broken = True
        verdict.errors.append(
            f"{len(verdict.reported)} shard(s) reported while others skipped for "
            "capacity — a rate here would cover only part of the diff"
        )

    return verdict


def main(argv):
    threshold = 80.0
    args = list(argv)
    require_override = "--require-override" in args
    if require_override:
        args.remove("--require-override")
    if "--threshold" in args:
        i = args.index("--threshold")
        threshold = float(args[i + 1])
        del args[i:i + 2]
    shards = 0
    if "--shards" in args:
        i = args.index("--shards")
        shards = int(args[i + 1])
        del args[i:i + 2]

    paths = sorted(p for arg in args for p in glob.glob(arg))
    if not paths:
        print("No shard logs found — nothing to aggregate")
        return 1
    # An artifact can expire or fail to upload, and one matrix child can be
    # retried on its own. Either way the missing shard's slice was never
    # measured, so the run cannot be scored as if it had been.
    if shards and len(paths) < shards:
        print(
            f"Only {len(paths)} of {shards} shard logs arrived — the missing "
            "shard(s) tested a slice of the diff nobody measured"
        )
        return 1

    contents = [open(p).read() for p in paths]
    verdict = aggregate(contents, names=paths)
    verdict.require_override = require_override
    verdict.override_text = os.environ.get("MUTATION_OVERRIDE_TEXT", "")
    verdict.override_truncated = (
        os.environ.get("MUTATION_OVERRIDE_TRUNCATED", "").lower() == "true"
    )
    print(verdict.render(threshold))
    return 1 if verdict.failed(threshold) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
