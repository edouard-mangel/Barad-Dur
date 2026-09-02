//! The field-test driver's pure decisions.
//!
//! `src/bin/field-test.rs` is gated behind the `field-test` cargo feature, so
//! nothing that lives there is compiled — let alone tested — by a plain
//! `cargo test`. Every decision the driver makes therefore lives here, in the
//! unconditionally-compiled library, where it is testable. Same pattern as
//! [`crate::field_test::mode`]; the driver is left as a loop that does I/O and
//! calls into this module.

use crate::field_test::audit::select_for_audit;
use crate::field_test::baseline::read_baseline;
use crate::field_test::mode::Mode;
use crate::field_test::surface::{ActionSurface, DecisionSurface};
use anyhow::Result;
use std::collections::BTreeSet;
use std::path::Path;

/// What the driver does with one corpus repository on this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoStep {
    /// Refuse to manufacture review evidence during a normal regression run.
    MissingBaseline,
    /// Diff the current surface against the committed baseline.
    Compare(DecisionSurface),
    /// Collect the repository for the audit worksheet. `None` means it has no
    /// baseline yet — every current recommendation is then *new*, which is
    /// exactly the first-ever-run scenario audit mode exists to catch.
    Audit(Option<DecisionSurface>),
    /// Overwrite the baseline unconditionally.
    Accept,
}

/// Decide what to do with one repository, given the mode and whether a
/// baseline was found.
///
/// Audit mode never writes a baseline: `accept` is the only path allowed to
/// change one, because that change has to land as its own reviewed commit.
/// A normal run without a baseline is an error rather than an implicit accept.
pub fn step_for(mode: Mode, baseline: Option<DecisionSurface>) -> RepoStep {
    match (mode, baseline) {
        (Mode::Accept, _) => RepoStep::Accept,
        (Mode::Audit, base) => RepoStep::Audit(base),
        (Mode::Run, None) => RepoStep::MissingBaseline,
        (Mode::Run, Some(base)) => RepoStep::Compare(base),
    }
}

/// Read the baseline this mode needs, if it needs one at all.
///
/// `accept` deliberately never reads: it overwrites whatever is on disk, and
/// it is precisely the command a maintainer reaches for to repair a corrupt
/// baseline. Reading first would let a parse error abort the only command
/// that can fix the file.
pub fn baseline_for(mode: Mode, dir: &Path, name: &str) -> Result<Option<DecisionSurface>> {
    match mode {
        Mode::Accept => Ok(None),
        Mode::Run | Mode::Audit => read_baseline(dir, name),
    }
}

/// One repository's contribution to the audit worksheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditInput {
    pub name: String,
    pub baseline: Option<DecisionSurface>,
    pub current: DecisionSurface,
}

/// Sample the corpus for the audit worksheet under **one shared** rotation
/// allowance.
///
/// The worksheet is bounded at roughly ten items per merge: every new or
/// changed recommendation (uncapped — those are the high-risk ones) plus a
/// rotating slice of at most `rotation` pre-existing ones *cycling across the
/// corpus*. `rotation` is therefore a corpus-wide budget, not a per-repository
/// one: spending it per repository multiplies it by the corpus size and turns
/// the mitigation for audit fatigue into its cause.
///
/// Once the allowance is exhausted, later repositories contribute no
/// pre-existing rows at all. That is correct: with no persisted rotation
/// state (see `docs/review-process.md`), the slice always starts at the front
/// of the corpus rather than genuinely cycling.
pub fn audit_corpus(
    inputs: &[AuditInput],
    already_seen: &BTreeSet<String>,
    rotation: usize,
) -> Vec<(String, Vec<ActionSurface>)> {
    inputs
        .iter()
        .scan(rotation, |remaining, input| {
            let baseline = input
                .baseline
                .clone()
                .unwrap_or_else(DecisionSurface::empty);
            let items = select_for_audit(&baseline, &input.current, already_seen, *remaining);
            let pre_existing: BTreeSet<&ActionSurface> = baseline.actions.iter().collect();
            let spent = items.iter().filter(|a| pre_existing.contains(a)).count();
            *remaining = remaining.saturating_sub(spent);
            Some((input.name.clone(), items))
        })
        .filter(|(_, items)| !items.is_empty())
        .collect()
}

/// The one-line summary a `run` prints when every comparison succeeded.
pub fn summary_line(compared: usize) -> String {
    format!("field test clean across {compared} repositories")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::baseline::baseline_path;

    fn with_actions(texts: &[&str]) -> DecisionSurface {
        DecisionSurface {
            actions: texts
                .iter()
                .map(|t| ActionSurface {
                    target_tab: "x".into(),
                    text: (*t).into(),
                })
                .collect(),
            ..DecisionSurface::empty()
        }
    }

    fn input(name: &str, baseline: Option<&[&str]>, current: &[&str]) -> AuditInput {
        AuditInput {
            name: name.to_string(),
            baseline: baseline.map(with_actions),
            current: with_actions(current),
        }
    }

    fn texts(sampled: &[(String, Vec<ActionSurface>)]) -> Vec<&str> {
        sampled
            .iter()
            .flat_map(|(_, items)| items.iter().map(|a| a.text.as_str()))
            .collect()
    }

    /// The bug this guards: `rotation` passed as a per-repository constant.
    /// With an 11-repo corpus that emits up to 55 pre-existing rows per run
    /// against a spec bound of ~10.
    #[test]
    fn rotation_allowance_is_spent_across_the_corpus_not_per_repository() {
        let inputs = vec![
            input(
                "first",
                Some(&["a1", "a2", "a3", "a4"]),
                &["a1", "a2", "a3", "a4"],
            ),
            input(
                "second",
                Some(&["b1", "b2", "b3", "b4"]),
                &["b1", "b2", "b3", "b4"],
            ),
            input(
                "third",
                Some(&["c1", "c2", "c3", "c4"]),
                &["c1", "c2", "c3", "c4"],
            ),
        ];
        let sampled = audit_corpus(&inputs, &BTreeSet::new(), 5);
        assert_eq!(
            texts(&sampled).len(),
            5,
            "the rotation budget is corpus-wide: 3 repos with 4 pre-existing \
             recommendations each must yield 5 rows in total, not 12, and not 5 per repo"
        );
    }

    /// The distinction that makes the budget safe: capping pre-existing rows
    /// must never cap new ones.
    #[test]
    fn new_recommendations_are_never_capped_by_the_rotation_allowance() {
        let inputs = vec![
            input("first", Some(&["a1", "a2"]), &["a1", "a2"]),
            input("second", Some(&["b1", "b2", "b3"]), &["b1", "b2", "b3"]),
            input(
                "third",
                Some(&["c1"]),
                &["c1", "new one", "new two", "new three"],
            ),
        ];
        let sampled = audit_corpus(&inputs, &BTreeSet::new(), 5);
        let seen = texts(&sampled);
        for fresh in ["new one", "new two", "new three"] {
            assert!(
                seen.contains(&fresh),
                "new recommendation {fresh:?} must appear even with the \
                 rotation allowance already exhausted, got: {seen:?}"
            );
        }
    }

    /// The first-ever run of a repository is the scenario audit mode exists
    /// for: with no baseline, every recommendation is new and all of them
    /// must reach the worksheet.
    #[test]
    fn a_repository_with_no_baseline_is_audited_against_an_empty_baseline() {
        let inputs = vec![input("brand-new", None, &["one", "two", "three"])];
        let sampled = audit_corpus(&inputs, &BTreeSet::new(), 0);
        assert_eq!(
            texts(&sampled),
            vec!["one", "two", "three"],
            "with no baseline every recommendation is new, so all of them are \
             sampled regardless of the rotation allowance"
        );
    }

    #[test]
    fn audit_mode_audits_a_repository_without_a_baseline_instead_of_seeding_it() {
        assert_eq!(
            step_for(Mode::Audit, None),
            RepoStep::Audit(None),
            "audit mode must audit a baseline-less repository, never seed it: \
             seeding writes the bad advice into the baseline unreviewed, which \
             is the exact failure audit mode exists to prevent"
        );
    }

    #[test]
    fn audit_mode_never_writes_a_baseline_even_when_one_exists() {
        let base = with_actions(&["a"]);
        assert_eq!(
            step_for(Mode::Audit, Some(base.clone())),
            RepoStep::Audit(Some(base)),
            "the only step allowed to write a baseline is Accept"
        );
    }

    #[test]
    fn run_mode_does_not_silently_seed_a_missing_baseline() {
        assert_eq!(step_for(Mode::Run, None), RepoStep::MissingBaseline);
    }

    #[test]
    fn run_mode_compares_against_an_existing_baseline() {
        let base = with_actions(&["a"]);
        assert_eq!(
            step_for(Mode::Run, Some(base.clone())),
            RepoStep::Compare(base)
        );
    }

    #[test]
    fn accept_overwrites_whether_or_not_a_baseline_was_found() {
        assert_eq!(step_for(Mode::Accept, None), RepoStep::Accept);
        assert_eq!(
            step_for(Mode::Accept, Some(with_actions(&["a"]))),
            RepoStep::Accept
        );
    }

    #[test]
    fn accept_does_not_read_the_existing_baseline_so_it_can_repair_a_corrupt_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(baseline_path(dir.path(), "corrupt"), "not valid json")
            .expect("write garbage");
        let got = baseline_for(Mode::Accept, dir.path(), "corrupt");
        assert!(
            got.is_ok(),
            "accept is the command that repairs a corrupt baseline; it must not \
             fail on parsing the file it is about to overwrite, got: {:?}",
            got.err().map(|e| format!("{e:#}"))
        );
    }

    #[test]
    fn run_still_fails_on_a_corrupt_baseline_rather_than_silently_reseeding() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(baseline_path(dir.path(), "corrupt"), "not valid json")
            .expect("write garbage");
        assert!(
            baseline_for(Mode::Run, dir.path(), "corrupt").is_err(),
            "run must still surface a corrupt baseline as an error"
        );
    }

    #[test]
    fn a_fully_compared_run_reports_the_corpus_size() {
        assert_eq!(summary_line(11), "field test clean across 11 repositories");
    }
}
