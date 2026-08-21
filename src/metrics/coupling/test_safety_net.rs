//! Source/test co-change "safety net" signal (Crime Scene Ch. 9): when a
//! source file's naming-convention-paired test file stops co-changing with
//! it, the safety net is eroding — the code moves, its tests don't. Reuses
//! `file_role::is_test_pair` (the same predicate the coupling-pair badge
//! uses) and the co-change ratio formula `qualifying_smell_pairs` already
//! uses, so no new derivation of "what counts as a meaningful pairing".

use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::CouplingThresholds;
use crate::metrics::file_role::{classify, is_test_pair, FileRole};
use crate::metrics::{score_count_bands, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

/// The strongest (highest co-change ratio) test-file candidate found for a
/// Source file.
struct TestPairing {
    test_path: PathBuf,
    co_change_ratio: f64,
}

/// For every Source-role file with a nonzero commit count and a
/// naming-convention candidate Test-role file in the repo, the strongest
/// (highest co-change ratio) candidate pairing found. A source file with no
/// candidate anywhere in `snapshot.files`, or with zero commits, is absent
/// from the map — "no test convention detected," never "coverage is bad"
/// (spec decision 3). A source file with a candidate but zero observed
/// co-changes is still present, with ratio `0.0` — it's *checked*, just
/// failing.
fn strongest_test_pairing(snapshot: &RepoSnapshot) -> HashMap<PathBuf, TestPairing> {
    // Co-change counts, indexed once: `file_change_pairs` stores each pair a
    // single time in lexicographic order, so the lookup key must match that.
    let co_changes: HashMap<(&PathBuf, &PathBuf), usize> = snapshot
        .file_change_pairs
        .iter()
        .map(|(a, b, count)| ((a, b), *count))
        .collect();

    let test_files: Vec<PathBuf> = snapshot
        .files
        .iter()
        .filter(|f| classify(&f.path) == FileRole::Test)
        .map(|f| f.path.clone())
        .collect();

    snapshot
        .files
        .iter()
        .filter(|f| classify(&f.path) == FileRole::Source)
        .filter_map(|source_file| {
            let source = &source_file.path;
            let commits_a = snapshot.commits_by_file.get(source).map_or(0, Vec::len);
            if commits_a == 0 {
                return None;
            }
            test_files
                .iter()
                .filter(|test| is_test_pair(source, test))
                .map(|test| {
                    let commits_b = snapshot.commits_by_file.get(test).map_or(0, Vec::len);
                    let (first, second) = if source < test {
                        (source, test)
                    } else {
                        (test, source)
                    };
                    let co = co_changes.get(&(first, second)).copied().unwrap_or(0);
                    let ratio = co as f64 / commits_a.min(commits_b).max(1) as f64;
                    TestPairing {
                        test_path: test.clone(),
                        co_change_ratio: ratio,
                    }
                })
                .max_by(|a, b| a.co_change_ratio.partial_cmp(&b.co_change_ratio).unwrap())
                .map(|pairing| (source.clone(), pairing))
        })
        .collect()
}

/// Pairs whose best ratio sits below `test_safety_net_min_ratio`: the
/// safety net is eroding for that source file. Scored on count via the
/// standard four-band scale (same as `change_coupling_smells`); evidence
/// lists the 10 worst pairs, ascending by ratio (worst first) then path.
pub(crate) fn test_safety_net(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> MetricValue {
    let pairings = strongest_test_pairing(snapshot);

    if pairings.is_empty() {
        return MetricValue {
            name: "Test safety net".to_string(),
            description: "No source/test pairs detected by naming convention".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let checked = pairings.len();
    let mut eroding: Vec<(&PathBuf, &TestPairing)> = pairings
        .iter()
        .filter(|(_, pairing)| pairing.co_change_ratio < thresholds.test_safety_net_min_ratio)
        .collect();
    eroding.sort_by(|(path_a, pairing_a), (path_b, pairing_b)| {
        pairing_a
            .co_change_ratio
            .partial_cmp(&pairing_b.co_change_ratio)
            .unwrap()
            .then_with(|| path_a.cmp(path_b))
    });
    let flagged = eroding.len();

    let evidence: Vec<String> = eroding
        .iter()
        .take(10)
        .map(|(source, pairing)| {
            format!(
                "{} ↔ {} — {:.0}% co-change",
                source.display(),
                pairing.test_path.display(),
                pairing.co_change_ratio * 100.0
            )
        })
        .collect();

    let threshold_pct = thresholds.test_safety_net_min_ratio * 100.0;
    let erosion_note = if flagged > 0 {
        " — safety net eroding"
    } else {
        ""
    };

    MetricValue {
        name: "Test safety net".to_string(),
        description: format!(
            "{flagged} of {checked} source/test pairs below {threshold_pct:.0}% co-change{erosion_note}"
        ),
        raw_value: RawValue::List(evidence),
        score: Some(score_count_bands(flagged)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::testutil::{make_file, make_snapshot};
    use crate::snapshot::CommitId;

    fn commits(n: u32) -> Vec<CommitId> {
        (0..n).map(CommitId).collect()
    }

    fn set_commits(snapshot: &mut RepoSnapshot, path: &str, n: u32) {
        snapshot
            .commits_by_file
            .insert(PathBuf::from(path), commits(n));
    }

    fn set_pair(snapshot: &mut RepoSnapshot, a: &str, b: &str, co_changes: usize) {
        let (a, b) = (PathBuf::from(a), PathBuf::from(b));
        let (first, second) = if a < b { (a, b) } else { (b, a) };
        snapshot.file_change_pairs.push((first, second, co_changes));
    }

    fn source_test_snapshot(
        source: &str,
        test: &str,
        commits_a: u32,
        commits_b: u32,
        co_changes: usize,
    ) -> RepoSnapshot {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![make_file(source), make_file(test)];
        set_commits(&mut snapshot, source, commits_a);
        set_commits(&mut snapshot, test, commits_b);
        set_pair(&mut snapshot, source, test, co_changes);
        snapshot
    }

    #[test]
    fn ratio_above_threshold_not_flagged() {
        let snapshot = source_test_snapshot("src/a.rs", "src/a_test.rs", 10, 10, 5);
        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(100));
        assert_eq!(
            result.description,
            "0 of 1 source/test pairs below 30% co-change"
        );
    }

    #[test]
    fn ratio_exactly_at_threshold_not_flagged() {
        // 3 / 10 == 0.30, the configured threshold — `<` is strict, so this
        // must NOT be flagged.
        let snapshot = source_test_snapshot("src/a.rs", "src/a_test.rs", 10, 10, 3);
        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(100));
        assert_eq!(
            result.description,
            "0 of 1 source/test pairs below 30% co-change"
        );
    }

    #[test]
    fn ratio_just_below_threshold_flagged() {
        // One co-change fewer than the boundary case above: 2 / 10 == 0.20.
        let snapshot = source_test_snapshot("src/a.rs", "src/a_test.rs", 10, 10, 2);
        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(75));
        assert_eq!(
            result.description,
            "1 of 1 source/test pairs below 30% co-change — safety net eroding"
        );
    }

    #[test]
    fn best_candidate_wins_only_flags_if_best_erodes() {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![
            make_file("src/foo.ts"),
            make_file("src/foo.test.ts"),
            make_file("src/foo.spec.ts"),
        ];
        set_commits(&mut snapshot, "src/foo.ts", 10);
        set_commits(&mut snapshot, "src/foo.test.ts", 10);
        set_commits(&mut snapshot, "src/foo.spec.ts", 10);
        // foo.test.ts drifted (ratio 0.1, would erode alone)...
        set_pair(&mut snapshot, "src/foo.ts", "src/foo.test.ts", 1);
        // ...but foo.spec.ts stayed tight (ratio 0.9) — the best candidate,
        // so the pairing is scored against it, not the drifted one.
        set_pair(&mut snapshot, "src/foo.ts", "src/foo.spec.ts", 9);

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(100));
        assert_eq!(
            result.description,
            "0 of 1 source/test pairs below 30% co-change"
        );
    }

    #[test]
    fn zero_co_changes_still_present_with_ratio_zero() {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![make_file("src/b.rs"), make_file("src/b_test.rs")];
        set_commits(&mut snapshot, "src/b.rs", 5);
        set_commits(&mut snapshot, "src/b_test.rs", 5);
        // No entry pushed into file_change_pairs at all: zero observed co-changes.

        let pairing = strongest_test_pairing(&snapshot);
        let entry = pairing.get(&PathBuf::from("src/b.rs")).unwrap();
        assert_eq!(entry.test_path, PathBuf::from("src/b_test.rs"));
        assert_eq!(entry.co_change_ratio, 0.0);

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(75));
        assert_eq!(
            result.description,
            "1 of 1 source/test pairs below 30% co-change — safety net eroding"
        );
    }

    #[test]
    fn source_with_no_candidate_is_absent() {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![
            make_file("src/a.rs"),
            make_file("src/a_test.rs"),
            make_file("src/lonely.rs"),
        ];
        set_commits(&mut snapshot, "src/a.rs", 10);
        set_commits(&mut snapshot, "src/a_test.rs", 10);
        set_commits(&mut snapshot, "src/lonely.rs", 10);
        set_pair(&mut snapshot, "src/a.rs", "src/a_test.rs", 5);

        let pairing = strongest_test_pairing(&snapshot);
        assert_eq!(pairing.len(), 1);
        assert!(!pairing.contains_key(&PathBuf::from("src/lonely.rs")));
    }

    #[test]
    fn source_with_zero_commits_is_absent() {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![make_file("src/a.rs"), make_file("src/a_test.rs")];
        // No commits_by_file entry for src/a.rs at all — zero commits.
        set_commits(&mut snapshot, "src/a_test.rs", 10);

        let pairing = strongest_test_pairing(&snapshot);
        assert!(pairing.is_empty());
    }

    #[test]
    fn score_count_bands_boundaries() {
        for &(flagged, expected_score) in &[
            (0usize, 100u32),
            (1, 75),
            (2, 75),
            (3, 50),
            (5, 50),
            (6, 25),
        ] {
            let mut snapshot = make_snapshot();
            let mut files = Vec::new();
            let n = flagged.max(1); // always at least one checked pair
            for i in 0..n {
                let source = format!("src/f{i:02}.rs");
                let test = format!("src/f{i:02}_test.rs");
                files.push(make_file(&source));
                files.push(make_file(&test));
                set_commits(&mut snapshot, &source, 10);
                set_commits(&mut snapshot, &test, 10);
                if i >= flagged {
                    // Only reached when flagged == 0: the one healthy pair
                    // that keeps this snapshot at "checked, not eroding".
                    set_pair(&mut snapshot, &source, &test, 9);
                }
                // else: no pair pushed — ratio 0.0, below threshold, eroding.
            }
            snapshot.files = files;
            let result = test_safety_net(&snapshot, &CouplingThresholds::default());
            assert_eq!(result.score, Some(expected_score), "flagged={flagged}");
        }
    }

    #[test]
    fn no_pairs_anywhere_returns_na() {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![make_file("src/lonely.rs")];
        set_commits(&mut snapshot, "src/lonely.rs", 10);
        // No Test-role file in the repo at all — no candidate anywhere.

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, None);
        assert_eq!(
            result.description,
            "No source/test pairs detected by naming convention"
        );
        assert!(matches!(result.raw_value, RawValue::Text(ref s) if s == "N/A"));
    }

    #[test]
    fn evidence_entry_format() {
        let snapshot = source_test_snapshot("src/a.rs", "tests/a_test.rs", 25, 25, 2);
        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert!(matches!(
            &result.raw_value,
            RawValue::List(list) if list == &vec!["src/a.rs ↔ tests/a_test.rs — 8% co-change".to_string()]
        ));
    }

    #[test]
    fn evidence_sorted_ascending_by_ratio() {
        let mut snapshot = make_snapshot();
        let mut files = Vec::new();
        // Distinct ratios pushed out of order (0.2, 0.0, 0.1) to prove the
        // metric sorts the evidence rather than preserving insertion order.
        let specs = [
            ("src/c.rs", "src/c_test.rs", 2usize),
            ("src/a.rs", "src/a_test.rs", 0),
            ("src/b.rs", "src/b_test.rs", 1),
        ];
        for (source, test, co_changes) in specs {
            files.push(make_file(source));
            files.push(make_file(test));
            set_commits(&mut snapshot, source, 10);
            set_commits(&mut snapshot, test, 10);
            set_pair(&mut snapshot, source, test, co_changes);
        }
        snapshot.files = files;

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        let expected = vec![
            "src/a.rs ↔ src/a_test.rs — 0% co-change".to_string(),
            "src/b.rs ↔ src/b_test.rs — 10% co-change".to_string(),
            "src/c.rs ↔ src/c_test.rs — 20% co-change".to_string(),
        ];
        assert!(matches!(&result.raw_value, RawValue::List(list) if list == &expected));
    }

    #[test]
    fn evidence_ties_broken_by_path() {
        let mut snapshot = make_snapshot();
        let mut files = Vec::new();
        // Both pairs have identical ratio 0.0 — tie-break must be by path.
        for (source, test) in [("src/z.rs", "src/z_test.rs"), ("src/a.rs", "src/a_test.rs")] {
            files.push(make_file(source));
            files.push(make_file(test));
            set_commits(&mut snapshot, source, 10);
            set_commits(&mut snapshot, test, 10);
        }
        snapshot.files = files;

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        let expected = vec![
            "src/a.rs ↔ src/a_test.rs — 0% co-change".to_string(),
            "src/z.rs ↔ src/z_test.rs — 0% co-change".to_string(),
        ];
        assert!(matches!(&result.raw_value, RawValue::List(list) if list == &expected));
    }

    #[test]
    fn evidence_capped_at_ten() {
        let mut snapshot = make_snapshot();
        let mut files = Vec::new();
        for i in 0..12u32 {
            let source = format!("src/f{i:02}.rs");
            let test = format!("src/f{i:02}_test.rs");
            files.push(make_file(&source));
            files.push(make_file(&test));
            set_commits(&mut snapshot, &source, 100);
            set_commits(&mut snapshot, &test, 100);
            set_pair(&mut snapshot, &source, &test, i as usize); // ratio i/100, all < 0.30
        }
        snapshot.files = files;

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(25)); // 12 flagged → the `_` band
        let RawValue::List(list) = &result.raw_value else {
            panic!("expected RawValue::List");
        };
        assert_eq!(list.len(), 10);
        assert_eq!(
            list.first().unwrap(),
            "src/f00.rs ↔ src/f00_test.rs — 0% co-change"
        );
        assert_eq!(
            list.last().unwrap(),
            "src/f09.rs ↔ src/f09_test.rs — 9% co-change"
        );
    }

    #[test]
    fn loosened_threshold_not_flagged() {
        // ratio 0.2 is flagged under the default 0.30 threshold...
        let snapshot = source_test_snapshot("src/a.rs", "src/a_test.rs", 10, 10, 2);
        let default_result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(default_result.score, Some(75));

        // ...but not once a team loosens the knob below that ratio.
        let loosened = CouplingThresholds {
            test_safety_net_min_ratio: 0.1,
            ..CouplingThresholds::default()
        };
        let loosened_result = test_safety_net(&snapshot, &loosened);
        assert_eq!(loosened_result.score, Some(100));
    }
}
