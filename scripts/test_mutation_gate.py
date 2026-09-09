"""Tests for the mutation kill-rate aggregator.

This script decides whether merge requests pass, so its edge cases —
skipped shards, broken shards, diffs with no Rust in them — are worth
pinning down. Log fixtures are trimmed copies of real cargo-mutants output.
"""

import unittest

from mutation_gate import Verdict, aggregate

CAUGHT_ONLY = "43 mutants tested in 73m: 33 caught, 9 unviable\n"
WITH_MISSED = "45 mutants tested in 2h: 1 missed, 39 caught, 5 unviable\n"
WITH_TIMEOUT = "10 mutants tested in 1h: 2 timeouts, 8 caught\n"
MOSTLY_MISSED = "10 mutants tested in 1h: 8 missed, 2 caught\n"


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

    def test_no_viable_mutants_is_a_pass(self):
        v = aggregate(["2 mutants tested in 1m: 2 unviable\n"])
        self.assertEqual(v.viable, 0)
        self.assertFalse(v.failed(80))


if __name__ == "__main__":
    unittest.main()
