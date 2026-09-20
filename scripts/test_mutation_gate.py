"""Tests for the mutation kill-rate aggregator.

This script decides whether merge requests pass, so its edge cases —
skipped shards, broken shards, diffs with no Rust in them — are worth
pinning down. Log fixtures are trimmed copies of real cargo-mutants output.
"""

import contextlib
import io
import os
import tempfile
import unittest
from unittest import mock

from mutation_gate import Verdict, aggregate, main

CAUGHT_ONLY = "43 mutants tested in 73m: 33 caught, 9 unviable\n"
WITH_MISSED = "45 mutants tested in 2h: 1 missed, 39 caught, 5 unviable\n"
WITH_TIMEOUT = "10 mutants tested in 1h: 2 timeouts, 8 caught\n"
MOSTLY_MISSED = "10 mutants tested in 1h: 8 missed, 2 caught\n"
OVER_CAPACITY = (
    "MUTATION OVER CAPACITY: 987 in-diff mutants exceed the per-MR capacity of 150\n"
)
OVERRIDE = (
    "## Summary\n\n## Mutation gate override\n\n"
    "Local campaign at acb0a91: kill rate 95.6%, every survivor dispositioned.\n"
)


def override(body):
    return f"## Summary\n\n## Mutation gate override\n\n{body}\n"


class AggregateTest(unittest.TestCase):
    def test_sums_counts_across_shards(self):
        v = aggregate([CAUGHT_ONLY, WITH_MISSED])
        self.assertEqual(v.tested, 88)
        self.assertEqual(v.caught, 72)
        self.assertEqual(v.missed, 1)
        self.assertEqual(v.unviable, 14)

    def test_kill_rate_excludes_unviable_from_the_denominator(self):
        # Unviable mutants do not compile, so no test could ever kill them.
        v = aggregate([CAUGHT_ONLY])
        self.assertEqual(v.viable, 34)
        self.assertAlmostEqual(v.rate, 33 / 34 * 100)

    def test_timeouts_count_as_caught(self):
        # A mutant that pushes the suite past 3x its baseline runtime is
        # exercised by tests and cannot ship undetected — it just fails slowly.
        v = aggregate([WITH_TIMEOUT])
        self.assertEqual(v.effective_caught, 10)
        self.assertEqual(v.rate, 100.0)

    def test_passes_at_or_above_the_threshold(self):
        self.assertFalse(aggregate([WITH_TIMEOUT]).failed(80))

    def test_fails_below_the_threshold(self):
        self.assertTrue(aggregate([MOSTLY_MISSED]).failed(80))

    def test_skipped_shards_are_ignored(self):
        v = aggregate(["skipping mutation run\n", CAUGHT_ONLY])
        self.assertEqual(v.tested, 43)

    def test_all_shards_skipped_is_a_pass_not_a_failure(self):
        v = aggregate(["skipping mutation run\n", "skipping mutation run\n"])
        self.assertTrue(v.all_skipped)
        self.assertFalse(v.failed(80))

    def test_a_diff_with_no_rust_is_a_skip(self):
        v = aggregate(["INFO Diff changes no Rust source files\n"])
        self.assertTrue(v.all_skipped)
        self.assertFalse(v.failed(80))

    def test_a_shard_whose_baseline_failed_is_an_error(self):
        v = aggregate(["ERROR cargo test failed in an unmutated tree\n"])
        self.assertTrue(v.broken)
        self.assertTrue(v.failed(80))

    def test_a_shard_with_no_summary_is_an_error(self):
        # Shards swallow cargo-mutants exit codes (|| true), so a log with no
        # summary means the run itself broke — a timeout, most likely.
        v = aggregate(["Found 3404 mutants to test\nTerminated\n"])
        self.assertTrue(v.broken)
        self.assertTrue(v.failed(80))

    def test_over_capacity_shards_report_the_mutant_count(self):
        v = aggregate([OVER_CAPACITY, OVER_CAPACITY])
        self.assertEqual(v.over_capacity, 987)
        self.assertIn("987", v.render(80))

    def test_over_capacity_alone_is_a_pass(self):
        """A main-branch pipeline has no description to carry an override; the
        nightly sweep covers the codebase there."""
        v = aggregate([OVER_CAPACITY])
        self.assertFalse(v.failed(80))

    def test_over_capacity_on_a_merge_request_needs_a_documented_override(self):
        v = aggregate([OVER_CAPACITY])
        v.require_override = True
        self.assertTrue(v.failed(80))
        self.assertIn("mutation-campaign.sh", v.render(80))

    def test_over_capacity_passes_with_a_documented_override(self):
        v = aggregate([OVER_CAPACITY])
        v.require_override = True
        v.override_text = OVERRIDE
        self.assertFalse(v.failed(80))
        # Not just the word "override": the FAIL rendering carries that too.
        self.assertIn("PASS", v.render(80))
        self.assertIn("95.6%", v.render(80))

    def test_a_run_mixing_over_capacity_and_real_results_is_an_error(self):
        """Every shard reads the same diff, so they are over capacity together
        or not at all. A mix means some shards were re-run under a different
        capacity, and scoring the ones that reported would rule on a fraction
        of the diff while looking like an ordinary verdict."""
        v = aggregate([OVER_CAPACITY] * 5 + [CAUGHT_ONLY])
        self.assertTrue(v.failed(80))
        self.assertIn("part of the diff", v.render(80))

    def test_a_truncated_description_is_named_as_the_reason(self):
        """GitLab stores only the first 2700 characters of a description in
        CI_MERGE_REQUEST_DESCRIPTION, so a section written past that point is
        invisible here. Saying "add an override section" to an author who wrote
        one sends them after the wrong problem."""
        v = aggregate([OVER_CAPACITY])
        v.require_override = True
        v.override_text = "## Summary\n\n" + "x" * 2600
        v.override_truncated = True
        self.assertTrue(v.failed(80))
        self.assertIn("truncated", v.render(80))

    def over_capacity_with(self, text):
        v = aggregate([OVER_CAPACITY])
        v.require_override = True
        v.override_text = text
        return v

    def test_a_heading_with_no_measurement_is_not_an_override(self):
        """The heading is what an author types; the kill rate and the commit it
        was measured at are what a reader can check afterwards."""
        v = self.over_capacity_with("## Mutation gate override\n")
        self.assertTrue(v.failed(80))

    def test_an_override_below_the_threshold_is_refused(self):
        v = self.over_capacity_with(override("Kill rate 12.0% at acb0a91."))
        self.assertTrue(v.failed(80))
        self.assertIn("12.0%", v.render(80))

    def test_an_override_needs_the_commit_it_was_measured_at(self):
        v = self.over_capacity_with(override("Kill rate 95.6%, all survivors reviewed."))
        self.assertTrue(v.failed(80))

    def test_a_capitalised_heading_is_still_an_override(self):
        """Headings in this repository's documents are capitalised as often as
        not, and refusing one costs the author a pipeline for nothing."""
        v = self.over_capacity_with(
            "## Mutation Gate Override\n\nKill rate 95.6% at acb0a91.\n"
        )
        self.assertFalse(v.failed(80))

    def test_an_override_inside_a_code_fence_does_not_count(self):
        v = self.over_capacity_with(
            "## Summary\n\n```\n## Mutation gate override\n\nKill rate 99.9% at acb0a91.\n```\n"
        )
        self.assertTrue(v.failed(80))

    def test_the_verdict_quotes_the_rate_and_commit_it_accepted(self):
        v = self.over_capacity_with(override("Kill rate 95.6% at acb0a91."))
        self.assertFalse(v.failed(80))
        self.assertIn("95.6%", v.render(80))
        self.assertIn("acb0a91", v.render(80))

    def test_an_override_section_in_the_description_does_not_excuse_a_low_rate(self):
        v = aggregate([MOSTLY_MISSED])
        v.require_override = True
        v.override_text = OVERRIDE
        self.assertTrue(v.failed(80))

    def test_no_viable_mutants_is_a_pass(self):
        v = aggregate(["2 mutants tested in 1m: 2 unviable\n"])
        self.assertEqual(v.viable, 0)
        self.assertFalse(v.failed(80))


class MainTest(unittest.TestCase):
    """main() is what CI calls, so the wiring from flags and environment to the
    verdict is worth pinning too."""

    def logs(self, content):
        directory = tempfile.mkdtemp()
        path = os.path.join(directory, "mutants-1.log")
        with open(path, "w") as handle:
            handle.write(content)
        return path

    def test_over_capacity_fails_a_merge_request_without_an_override(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(
                main([self.logs(OVER_CAPACITY), "--require-override"]), 1
            )

    def test_over_capacity_passes_with_the_override_in_the_environment(self):
        with mock.patch.dict(os.environ, {"MUTATION_OVERRIDE_TEXT": OVERRIDE}):
            self.assertEqual(
                main([self.logs(OVER_CAPACITY), "--require-override"]), 0
            )

    def test_over_capacity_passes_outside_a_merge_request(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(main([self.logs(OVER_CAPACITY)]), 0)

    def test_a_missing_shard_log_is_an_error(self):
        """Artifacts can expire or fail to upload, and a single matrix child
        can be retried on its own. Scoring whatever logs turned up would give
        a confident rate for a diff that was only partly measured."""
        printed = io.StringIO()
        with mock.patch.dict(os.environ, {}, clear=True):
            with contextlib.redirect_stdout(printed):
                code = main([self.logs(CAUGHT_ONLY), "--shards", "6"])
        self.assertEqual(code, 1)
        self.assertIn("1 of 6", printed.getvalue())

    def test_every_shard_reporting_is_not_an_error(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(main([self.logs(CAUGHT_ONLY), "--shards", "1"]), 0)

    def test_a_truncated_description_reaches_the_verdict_from_the_environment(self):
        environment = {
            "MUTATION_OVERRIDE_TEXT": "## Summary",
            "MUTATION_OVERRIDE_TRUNCATED": "true",
        }
        printed = io.StringIO()
        with mock.patch.dict(os.environ, environment, clear=True):
            with contextlib.redirect_stdout(printed):
                code = main([self.logs(OVER_CAPACITY), "--require-override"])
        self.assertEqual(code, 1)
        self.assertIn("truncated", printed.getvalue())

    def test_a_low_rate_still_fails(self):
        with mock.patch.dict(os.environ, {"MUTATION_OVERRIDE_TEXT": OVERRIDE}):
            self.assertEqual(
                main([self.logs(MOSTLY_MISSED), "--require-override"]), 1
            )


if __name__ == "__main__":
    unittest.main()
