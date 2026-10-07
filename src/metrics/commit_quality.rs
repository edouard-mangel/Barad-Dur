//! How good one commit message is, on a 0–100 scale. Feeds the per-author
//! `avg_commit_quality`; the hygiene category's pass/fail "Commit message
//! quality" metric has its own rule in `hygiene.rs`.

/// Score one commit message: 10 for having one, plus a length ladder, a
/// conventional-prefix bonus, and a bonus for a body or a longer subject.
pub fn score_commit_message(msg: &str) -> f64 {
    let trimmed = msg.trim();
    let len = trimmed.len();

    let mut score: f64 = 10.0; // base points for having any message

    // Length score: 0-40 points
    score += match len {
        0..=3 => 0.0,
        4..=10 => 10.0,
        11..=50 => 30.0,
        _ => 40.0,
    };

    // Conventional commit prefix: +30 points
    let prefixes = [
        "feat:",
        "fix:",
        "docs:",
        "style:",
        "refactor:",
        "perf:",
        "test:",
        "chore:",
        "ci:",
        "build:",
    ];
    if prefixes.iter().any(|p| trimmed.starts_with(p)) {
        score += 30.0;
    }

    // Descriptive (>20 chars or has body): +20 points
    if trimmed.contains('\n') || len > 20 {
        score += 20.0;
    }

    // Penalty for low-effort messages
    let lower = trimmed.to_lowercase();
    if lower == "wip" || lower == "fix" || lower == "update" || lower == "." {
        score = score.min(10.0);
    }

    score.min(100.0)
}

#[cfg(test)]
mod tests {
    use super::score_commit_message;

    // Expected values are worked out by hand from the rule, not read back
    // from the function: 10 for having a message, plus 0/10/30/40 by length
    // (0-3 / 4-10 / 11-50 / more), plus 30 for a lowercase `type:` prefix,
    // plus 20 for a body or more than 20 characters; a low-effort message
    // is held to 10; the total is capped at 100.

    #[test]
    fn scores_follow_the_additive_rule() {
        let table = [
            ("feat: add login flow with validation", 90.0), // 10 + 30 + 30 + 20
            ("fix: typo", 50.0),                            // 10 + 10 + 30
            ("chore: bump\n\nnotes", 90.0),                 // 10 + 30 + 30 + 20 (body)
            ("  fix: typo  ", 50.0),                        // trimmed first
        ];
        for (message, expected) in table {
            assert_eq!(score_commit_message(message), expected, "{message:?}");
        }
    }

    #[test]
    fn length_buckets_change_exactly_at_their_edges() {
        // No prefix, no body: 10 + bucket, plus 20 once longer than 20.
        let table = [
            (3, 10.0),  // 0..=3   -> +0
            (4, 20.0),  // 4..=10  -> +10
            (10, 20.0), //
            (11, 40.0), // 11..=50 -> +30
            (20, 40.0), //   (20 is not yet "descriptive")
            (21, 60.0), //   +20 for more than 20
            (50, 60.0), //
            (51, 70.0), // 51+     -> +40, +20
        ];
        for (len, expected) in table {
            let message = "a".repeat(len);
            assert_eq!(score_commit_message(&message), expected, "length {len}");
        }
    }

    #[test]
    fn every_listed_prefix_earns_the_conventional_bonus() {
        // "<prefix> ab": 10 + length bucket + 30. All are 7-9 characters
        // (bucket +10 -> 50) except `refactor:` at 12 (bucket +30 -> 70).
        let table = [
            ("feat:", 50.0),
            ("fix:", 50.0),
            ("docs:", 50.0),
            ("style:", 50.0),
            ("refactor:", 70.0),
            ("perf:", 50.0),
            ("test:", 50.0),
            ("chore:", 50.0),
            ("ci:", 50.0),
            ("build:", 50.0),
        ];
        for (prefix, expected) in table {
            assert_eq!(
                score_commit_message(&format!("{prefix} ab")),
                expected,
                "{prefix}"
            );
        }
    }

    #[test]
    fn a_long_conventional_message_is_capped_at_one_hundred() {
        let message = format!("feat: {}", "x".repeat(60)); // 10 + 40 + 30 + 20
        assert_eq!(score_commit_message(&message), 100.0);
    }

    #[test]
    fn low_effort_messages_are_held_to_ten() {
        for message in ["wip", "update", ".", "WIP", "Fix"] {
            assert_eq!(score_commit_message(message), 10.0, "{message:?}");
        }
        assert_eq!(score_commit_message(""), 10.0);
    }

    #[test]
    fn only_a_lowercase_unscoped_prefix_earns_the_conventional_bonus() {
        // Characterization of a known divergence, not an endorsement: the
        // hygiene metric counts `fix(scope):` and `Fix:` as conventional
        // (case-insensitive, scoped forms, `revert:`), so the same commit is
        // conventional in one view and unremarkable in the other. Unifying
        // the two changes reported author scores, which is a product
        // decision tracked in docs/plans/refactor-preservation-ledger.md.
        assert_eq!(score_commit_message("fix: typo"), 50.0);
        assert_eq!(score_commit_message("fix(gate): typo"), 40.0); // 10 + 30, no bonus
        assert_eq!(score_commit_message("Fix: typo"), 20.0); // 10 + 10, no bonus
    }
}
