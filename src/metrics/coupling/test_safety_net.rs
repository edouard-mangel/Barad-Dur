//! Source/test co-change "safety net" signal (Crime Scene Ch. 9): when a
//! source file's naming-convention-paired test file stops co-changing with
//! it, the safety net is eroding — the code moves, its tests don't. Reuses
//! `file_role::is_test_pair` (the same predicate the coupling-pair badge
//! uses) and the co-change ratio formula `qualifying_smell_pairs` already
//! uses, so no new derivation of "what counts as a meaningful pairing".

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::CouplingThresholds;
use crate::metrics::file_role::{classify, is_test_pair, pair_stem, FileRole};
use crate::metrics::{score_prevalence, MetricValue, RawValue};
use crate::snapshot::{CommitId, RepoSnapshot};

/// The strongest (highest co-change ratio) test-file candidate found for a
/// Source file.
struct TestPairing {
    test_path: PathBuf,
    co_change_ratio: f64,
}

fn language_family(path: &Path) -> &str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => "js",
        "c" | "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => "c",
        "java" | "kt" => "jvm",
        other => other,
    }
}

fn first_component(path: &Path) -> Option<&std::ffi::OsStr> {
    path.components().next().map(|part| part.as_os_str())
}

fn project_scope(path: &Path) -> String {
    let parts: Vec<_> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect();
    if parts.len() <= 1 {
        return ".".to_string();
    }
    if let Some(src) = parts.iter().position(|part| part == "src") {
        return if src == 0 {
            ".".to_string()
        } else {
            parts[..src].join("/")
        };
    }
    // A workspace marker dir scopes to the member right after it, wherever
    // the marker sits — at the repo root (packages/arc/…) or nested
    // (front/packages/arc/…).
    let is_marker = |part: &str| {
        matches!(
            part,
            "packages" | "frontends" | "apps" | "services" | "libs"
        )
    };
    if let Some(pos) = parts.iter().position(|part| is_marker(part)) {
        if parts.len() > pos + 2 {
            return parts[..pos + 2].join("/");
        }
    }
    parts
        .first()
        .map_or_else(|| ".".to_string(), ToString::to_string)
}

fn same_project_scope(source: &Path, test: &Path) -> bool {
    let common_root_dir = |name: &std::ffi::OsStr| {
        matches!(
            name.to_str().unwrap_or_default(),
            "src" | "test" | "tests" | "spec" | "specs"
        )
    };
    let test_root_dir = |name: &std::ffi::OsStr| {
        matches!(
            name.to_str().unwrap_or_default(),
            "test" | "tests" | "spec" | "specs"
        )
    };
    match (first_component(source), first_component(test)) {
        (Some(a), Some(b)) if common_root_dir(a) && common_root_dir(b) => true,
        // A test tree at the repository root (tests/, spec/, …) serves the
        // whole repo: any root package dir (app/, lib/, internal/, …)
        // pairs with it.
        (Some(_), Some(b)) if test_root_dir(b) => true,
        (Some(a), Some(b)) => {
            (a == b || (project_scope(source) == "." && project_scope(test) == "."))
                && project_scope(source) == project_scope(test)
        }
        _ => true,
    }
}

fn compatible_pair(source: &Path, test: &Path) -> bool {
    language_family(source) == language_family(test) && same_project_scope(source, test)
}

/// Exact co-change count between two files: the size of the intersection of
/// their `commits_by_file` commit-id lists. Deliberately NOT sourced from
/// `snapshot.file_change_pairs` — that table only retains pairs whose count
/// already reaches `count_co_changed_pairs`'s built-in floor of 3 (the
/// change-coupling-smell threshold), so a real, healthy pair sitting at 1 or
/// 2 co-changes is silently absent from it. This metric's "absent" already
/// means "no naming-convention candidate at all"; treating the floored
/// table's absence as "zero co-changes" too would falsely read a healthy,
/// under-the-floor pair as a fully eroded safety net — the floor is
/// false-positive-biased for exactly this metric. Counts include merge
/// commits, the same inclusion semantics `commits_by_file` (and the floored
/// pair table) already have — neither index filters merges out.
fn co_change_count(commits_a: &[CommitId], commits_b: &[CommitId]) -> usize {
    // Always hash `a` and probe with `b` — the previous shorter/longer
    // selection was a provably output-equivalent branch (the intersection
    // size is the same either way), which makes it an equivalent-mutant
    // magnet: nothing observable distinguishes "hash the shorter side" from
    // "hash `a`", so a mutant flipping the comparison can never be killed.
    // Dropped post-final-review; the extra allocation is negligible here.
    let a_set: HashSet<CommitId> = commits_a.iter().copied().collect();
    commits_b.iter().filter(|c| a_set.contains(c)).count()
}

/// The 6 bare suffix forms `is_test_of(_, source_stem)` can hold under —
/// i.e. `source_stem` itself equals `{base}{suffix}` for one of these — used
/// by the reverse-direction half of `candidate_stems` below. Kept in
/// lockstep with `file_role::is_test_of`'s test-argument forms (`test`,
/// `tests`, `.test`, `.spec`, `_test`, `_spec`); the 7th form, the `test_`
/// prefix, is handled separately since it strips from the front.
const REVERSE_STRIP_SUFFIXES: &[&str] = &["tests", "test", ".test", ".spec", "_test", "_spec"];
const REVERSE_STRIP_PREFIX: &str = "test_";

/// Every stem `is_test_pair(source, candidate)` can match against, for a
/// source whose lowercase pair-stem is `source_stem` — the full, provably
/// symmetric closure of `is_test_of(sa, sb) || is_test_of(sb, sa)` restricted
/// to `sa = source_stem`, split into its two directions:
///
/// - Forward (`is_test_of(sa, sb)`, `sb` unknown): the candidate's stem must
///   be exactly one of the 7 forms of `source_stem` — `{s}test`, `{s}tests`,
///   `{s}.test`, `{s}.spec`, `{s}_test`, `{s}_spec`, `test_{s}`. These are
///   looked up directly.
/// - Reverse (`is_test_of(sb, sa)`, `sb` unknown): `source_stem` itself must
///   equal one of those same 7 forms applied to the (unknown) candidate
///   stem `sb` — so `sb` is recovered by *stripping* each form's
///   suffix/prefix from `source_stem` wherever it's actually present.
///   Each successful strip yields one more candidate stem to look up (up to
///   7 extra, one per form); a form that doesn't match `source_stem`
///   contributes nothing.
///
/// Together these two directions cover exactly what a naive `is_test_pair`
/// scan over every Test-role file would find — proven per-call by the
/// `debug_assert!` cross-check in `strongest_test_pairing` — while staying
/// O(1) lookups per source instead of an O(test files) scan.
fn candidate_stems(source_stem: &str) -> Vec<String> {
    let mut stems = vec![
        format!("{source_stem}test"),
        format!("{source_stem}tests"),
        format!("{source_stem}.test"),
        format!("{source_stem}.spec"),
        format!("{source_stem}_test"),
        format!("{source_stem}_spec"),
        format!("test_{source_stem}"),
    ];

    for suffix in REVERSE_STRIP_SUFFIXES {
        if let Some(base) = source_stem.strip_suffix(suffix) {
            stems.push(base.to_string());
        }
    }
    if let Some(base) = source_stem.strip_prefix(REVERSE_STRIP_PREFIX) {
        stems.push(base.to_string());
    }

    stems
}

/// Cross-check for the debug-only guard in `strongest_test_pairing`: the
/// stem-index lookup must find exactly the same candidates a naive
/// `is_test_pair` scan over every Test-role file would find. Returns `None`
/// when they match, otherwise a diagnostic naming the symmetric difference
/// — candidates the index found that the scan didn't (should never happen;
/// would mean a false-positive form) and candidates the scan found that the
/// index missed (the class of bug this guard exists to catch). Only ever
/// called from behind `cfg!(debug_assertions)` (a no-op in release builds),
/// so this O(test files) scan never runs in production.
fn indexed_candidates_diff(
    source: &Path,
    indexed: &[&PathBuf],
    test_files: &[&PathBuf],
) -> Option<String> {
    let indexed_set: HashSet<&PathBuf> = indexed.iter().copied().collect();
    let scanned_set: HashSet<&PathBuf> = test_files
        .iter()
        .copied()
        .filter(|test| is_test_pair(source, test) && compatible_pair(source, test))
        .collect();

    if indexed_set == scanned_set {
        return None;
    }

    let index_only: Vec<&PathBuf> = indexed_set.difference(&scanned_set).copied().collect();
    let scan_only: Vec<&PathBuf> = scanned_set.difference(&indexed_set).copied().collect();
    Some(format!(
        "stem-index candidates diverge from is_test_pair scan for {source:?}: \
         index-only (unexpected — a false-positive form) = {index_only:?}, \
         scan-only (missed by the index) = {scan_only:?}"
    ))
}

/// For every Source-role file with a nonzero commit count and a
/// naming-convention candidate Test-role file in the repo, the strongest
/// (highest co-change ratio) candidate pairing found. A source file with no
/// candidate anywhere in `snapshot.files`, or with zero commits, is absent
/// from the map — "no test convention detected," never "coverage is bad"
/// (spec decision 3). A source file with a candidate but zero observed
/// co-changes is still present, with ratio `0.0` — it's *checked*, just
/// failing.
///
/// Candidate discovery is index-based rather than an O(sources × test
/// files) scan through `is_test_pair` (which itself does ~16 heap
/// allocations per probe): Test-role files are indexed once by lowercase
/// pair-stem, then each source derives up to 14 candidate stems
/// (`candidate_stems`: 7 forward forms plus up to 7 reverse strips) and does
/// that many direct map lookups — O(sources) lookups total (a small,
/// bounded constant per source), regardless of repo size.
fn strongest_test_pairing(snapshot: &RepoSnapshot) -> HashMap<PathBuf, TestPairing> {
    let test_files: Vec<&PathBuf> = snapshot
        .files
        .iter()
        .filter(|f| classify(&f.path) == FileRole::Test)
        .map(|f| &f.path)
        .collect();

    let mut test_index: HashMap<String, Vec<&PathBuf>> = HashMap::new();
    for path in &test_files {
        if let Some(name) = path.to_str() {
            test_index
                .entry(pair_stem(name).to_lowercase())
                .or_default()
                .push(path);
        }
    }

    snapshot
        .files
        .iter()
        .filter(|f| classify(&f.path) == FileRole::Source)
        .filter_map(|source_file| {
            let source = &source_file.path;
            let commits_a = snapshot
                .commits_by_file
                .get(source)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if commits_a.is_empty() {
                return None;
            }
            let source_stem = pair_stem(source.to_str()?).to_lowercase();

            let candidates: Vec<&PathBuf> = candidate_stems(&source_stem)
                .iter()
                .filter_map(|stem| test_index.get(stem))
                .flatten()
                .copied()
                .filter(|test| compatible_pair(source, test))
                .collect();

            // Manually gated on the same flag `debug_assert!` uses (rather
            // than `debug_assert!(indexed_candidates_diff(..).is_none())`)
            // so the diff — and the format work to render it — is computed
            // at most once, only on divergence, only in debug builds.
            if cfg!(debug_assertions) {
                if let Some(diff) = indexed_candidates_diff(source, &candidates, &test_files) {
                    panic!("{diff}");
                }
            }

            candidates
                .iter()
                .map(|test| {
                    let commits_b = snapshot
                        .commits_by_file
                        .get(*test)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let co = co_change_count(commits_a, commits_b);
                    let ratio = co as f64 / commits_a.len().min(commits_b.len()).max(1) as f64;
                    TestPairing {
                        test_path: (*test).clone(),
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
        score: Some(score_prevalence(flagged, checked)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::testutil::{make_file, make_snapshot};

    #[test]
    fn compatibility_rejects_cross_project_and_cross_language_matches() {
        assert!(!compatible_pair(
            Path::new("api/app/Formatters.php"),
            Path::new("front/tests/Formatters.spec.tsx")
        ));
        assert!(!compatible_pair(
            Path::new("api/app/User.php"),
            Path::new("template/api/tests/UserTest.php")
        ));
        assert!(compatible_pair(
            Path::new("api/app/User.php"),
            Path::new("api/tests/UserTest.php")
        ));
        assert!(compatible_pair(
            Path::new("src/parser.ts"),
            Path::new("tests/parser.spec.tsx")
        ));
        assert!(!compatible_pair(
            Path::new("front/packages/arc/src/components/index.ts"),
            Path::new("front/packages/auth/src/index.spec.ts")
        ));
    }

    #[test]
    fn compatibility_accepts_root_level_package_and_test_dirs() {
        // The standard single-project layouts: a package dir at the root
        // paired with a root-level test tree (Django app/ + tests/, Ruby
        // lib/ + spec/, Go internal/ + tests/).
        assert!(compatible_pair(
            Path::new("app/models.py"),
            Path::new("tests/test_models.py")
        ));
        assert!(compatible_pair(
            Path::new("lib/user.rb"),
            Path::new("spec/user_spec.rb")
        ));
        assert!(compatible_pair(
            Path::new("internal/parser.go"),
            Path::new("tests/parser_test.go")
        ));
    }

    #[test]
    fn compatibility_accepts_root_level_monorepo_workspace_pairs() {
        // Root-level workspace markers (the default pnpm/yarn layout) must
        // scope to the workspace member, same as the nested front/packages
        // form already does.
        assert!(compatible_pair(
            Path::new("packages/arc/src/Button.tsx"),
            Path::new("packages/arc/tests/Button.spec.tsx")
        ));
        assert!(!compatible_pair(
            Path::new("packages/arc/src/Button.tsx"),
            Path::new("packages/auth/tests/Button.spec.tsx")
        ));
        assert!(compatible_pair(
            Path::new("services/auth/src/session.go"),
            Path::new("services/auth/tests/session_test.go")
        ));
        assert!(compatible_pair(
            Path::new("apps/web/src/router.ts"),
            Path::new("apps/web/tests/router.spec.ts")
        ));
        // A marker dir holding files directly (no member dir after it) must
        // not consume the filename as a workspace member.
        assert!(compatible_pair(
            Path::new("packages/util.ts"),
            Path::new("packages/util.spec.ts")
        ));
    }

    #[test]
    fn compatibility_accepts_cross_extension_pairs_within_language_family() {
        // C/C++ headers tested by .cpp files and Java code tested in Kotlin
        // are the same language family; unrelated languages still reject.
        assert!(compatible_pair(
            Path::new("include/foo.hpp"),
            Path::new("tests/foo_test.cpp")
        ));
        assert!(compatible_pair(
            Path::new("src/foo.h"),
            Path::new("tests/foo_test.c")
        ));
        assert!(compatible_pair(
            Path::new("src/User.java"),
            Path::new("test/UserTest.kt")
        ));
        assert!(!compatible_pair(
            Path::new("src/user.py"),
            Path::new("tests/user_test.rb")
        ));
    }

    fn ids(values: &[u32]) -> Vec<CommitId> {
        values.iter().copied().map(CommitId).collect()
    }

    /// A single file's raw commit count, with arbitrary (non-overlapping
    /// with anything) commit ids — used when the exact co-change count
    /// with a partner doesn't matter, only "has commits" or "has none".
    fn set_commits(snapshot: &mut RepoSnapshot, path: &str, n: u32) {
        let commit_ids: Vec<u32> = (9_000_000..9_000_000 + n).collect();
        snapshot
            .commits_by_file
            .insert(PathBuf::from(path), ids(&commit_ids));
    }

    /// Sets both files' full `commits_by_file` entries so that exactly
    /// `shared` commit ids are common to both, out of `commits_a` and
    /// `commits_b` total commits respectively — the co-change count the
    /// production code will recompute via set intersection.
    /// `snapshot.file_change_pairs` is deliberately never touched by any
    /// fixture in this module, to prove the metric doesn't read it.
    fn set_shared_commits(
        snapshot: &mut RepoSnapshot,
        source: &str,
        test: &str,
        commits_a: u32,
        commits_b: u32,
        shared: u32,
    ) {
        assert!(shared <= commits_a && shared <= commits_b);
        let mut a_ids: Vec<u32> = (0..shared).collect();
        a_ids.extend(1_000_000..1_000_000 + (commits_a - shared));
        let mut b_ids: Vec<u32> = (0..shared).collect();
        b_ids.extend(2_000_000..2_000_000 + (commits_b - shared));
        snapshot
            .commits_by_file
            .insert(PathBuf::from(source), ids(&a_ids));
        snapshot
            .commits_by_file
            .insert(PathBuf::from(test), ids(&b_ids));
    }

    fn source_test_snapshot(
        source: &str,
        test: &str,
        commits_a: u32,
        commits_b: u32,
        shared: u32,
    ) -> RepoSnapshot {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![make_file(source), make_file(test)];
        set_shared_commits(&mut snapshot, source, test, commits_a, commits_b, shared);
        snapshot
    }

    #[test]
    fn app_plus_tests_layout_is_checked_not_dark() {
        // Regression: the app/ + tests/ layout must produce a checked (and
        // here eroding) pairing, not "No source/test pairs detected".
        let snapshot = source_test_snapshot("app/models.py", "tests/test_models.py", 10, 10, 0);
        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(75));
        assert_eq!(
            result.description,
            "1 of 1 source/test pairs below 30% co-change — safety net eroding"
        );
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
        // foo.ts's 10 commits split exactly: 1 shared only with
        // foo.test.ts, 9 shared only with foo.spec.ts.
        snapshot.commits_by_file.insert(
            PathBuf::from("src/foo.ts"),
            ids(&[100, 200, 201, 202, 203, 204, 205, 206, 207, 208]),
        );
        // foo.test.ts drifted (ratio 0.1, would erode alone): the 1 shared
        // commit plus 9 unrelated ones.
        snapshot.commits_by_file.insert(
            PathBuf::from("src/foo.test.ts"),
            ids(&[100, 300, 301, 302, 303, 304, 305, 306, 307, 308]),
        );
        // foo.spec.ts stayed tight (ratio 0.9) — the best candidate, so
        // the pairing is scored against it, not the drifted one.
        snapshot.commits_by_file.insert(
            PathBuf::from("src/foo.spec.ts"),
            ids(&[200, 201, 202, 203, 204, 205, 206, 207, 208, 400]),
        );

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(100));
        assert_eq!(
            result.description,
            "0 of 1 source/test pairs below 30% co-change"
        );
    }

    #[test]
    fn zero_co_changes_still_present_with_ratio_zero() {
        let snapshot = source_test_snapshot("src/b.rs", "src/b_test.rs", 5, 5, 0);

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
        let mut snapshot = source_test_snapshot("src/a.rs", "src/a_test.rs", 10, 10, 5);
        snapshot.files.push(make_file("src/lonely.rs"));
        set_commits(&mut snapshot, "src/lonely.rs", 10);

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
                let shared = if i < flagged {
                    0 // ratio 0.0, below threshold, eroding
                } else {
                    // Only reached when flagged == 0: the one healthy pair
                    // that keeps this snapshot at "checked, not eroding".
                    9
                };
                set_shared_commits(&mut snapshot, &source, &test, 10, 10, shared);
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
        // Distinct ratios set up out of order (0.2, 0.0, 0.1) to prove the
        // metric sorts the evidence rather than preserving insertion order.
        let specs = [
            ("src/c.rs", "src/c_test.rs", 2u32),
            ("src/a.rs", "src/a_test.rs", 0u32),
            ("src/b.rs", "src/b_test.rs", 1u32),
        ];
        for (source, test, shared) in specs {
            files.push(make_file(source));
            files.push(make_file(test));
            set_shared_commits(&mut snapshot, source, test, 10, 10, shared);
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
            set_shared_commits(&mut snapshot, source, test, 10, 10, 0);
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
            set_shared_commits(&mut snapshot, &source, &test, 100, 100, i); // ratio i/100, all < 0.30
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

    #[test]
    fn co_change_count_uses_exact_intersection_not_floored_pair_table() {
        // Regression (coordinator finding): `file_change_pairs` only keeps
        // pairs whose count reaches `count_co_changed_pairs`'s built-in
        // floor of 3. 3 commits per file, 2 shared: true ratio 2/3 ≈ 0.667,
        // comfortably above the 0.30 default threshold — must NOT be
        // flagged. Below the floor, `file_change_pairs` would never contain
        // this pair at all, so a lookup sourced from that table reads it as
        // zero co-changes and falsely flags a healthy pair.
        let snapshot = source_test_snapshot("src/a.rs", "src/a_test.rs", 3, 3, 2);
        assert!(
            snapshot.file_change_pairs.is_empty(),
            "fixture must never populate file_change_pairs — proves the metric doesn't read it"
        );

        let pairing = strongest_test_pairing(&snapshot);
        let entry = pairing.get(&PathBuf::from("src/a.rs")).unwrap();
        assert!((entry.co_change_ratio - 2.0 / 3.0).abs() < 1e-9);

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(100));
        assert_eq!(
            result.description,
            "0 of 1 source/test pairs below 30% co-change"
        );
    }

    #[test]
    fn ratio_uses_min_commit_count_as_denominator_regardless_of_which_side_is_larger() {
        // 1 shared commit, source has 10, test has 4: the ratio must divide
        // by the SMALLER count (4), giving 1/4 == 0.25 — below the 0.30
        // threshold, flagged. A min→max mutant would divide by 10 instead
        // (1/10 == 0.10), which the "25% co-change" assertion below catches.
        let snapshot = source_test_snapshot("src/a.rs", "src/a_test.rs", 10, 4, 1);
        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(result.score, Some(75));
        assert_eq!(
            result.description,
            "1 of 1 source/test pairs below 30% co-change — safety net eroding"
        );
        let RawValue::List(list) = &result.raw_value else {
            panic!("expected RawValue::List");
        };
        assert_eq!(
            list,
            &vec!["src/a.rs ↔ src/a_test.rs — 25% co-change".to_string()]
        );

        // Swap which file has more commits (source 4, test 10, still 1
        // shared): `min` is symmetric, so the ratio must land on the exact
        // same 25% — a mutant that instead always divides by whichever
        // argument the code happens to name (e.g. always `commits_a.len()`
        // rather than the true min) would produce a different value here
        // (1/4 in the first case, 1/4 again — same — vs. a mutant using the
        // *other* argument, which would show 10% here instead), so this
        // second direction kills mutants the first direction alone can't.
        let swapped = source_test_snapshot("src/a.rs", "src/a_test.rs", 4, 10, 1);
        let swapped_result = test_safety_net(&swapped, &CouplingThresholds::default());
        assert_eq!(swapped_result.score, Some(75));
        let RawValue::List(swapped_list) = &swapped_result.raw_value else {
            panic!("expected RawValue::List");
        };
        assert_eq!(swapped_list, list);
    }

    #[test]
    fn reverse_direction_bare_concatenation_pairs_via_test_dir_candidate() {
        // Regression (scoped re-review counterexample): source stem
        // "usertest" is a bare-concatenation test-form of "user"
        // (`is_test_of("user", "usertest")` is true) but has no separator,
        // so `has_test_name` does not classify "usertest.rs" as Test — it
        // stays Source. Its candidate, "user.rs", is Test-role only
        // because it lives in a `tests/` directory, not because of its own
        // stem. The naive `is_test_pair` scan finds this pair via its
        // `is_test_of(sb, sa)` branch; the stem index must find it too via
        // the reverse-strip derivation, not just the forward 7 forms.
        let snapshot = source_test_snapshot("src/usertest.rs", "tests/user.rs", 10, 10, 5);

        let pairing = strongest_test_pairing(&snapshot);
        let entry = pairing
            .get(&PathBuf::from("src/usertest.rs"))
            .expect("reverse-direction pairing must be found");
        assert_eq!(entry.test_path, PathBuf::from("tests/user.rs"));

        let result = test_safety_net(&snapshot, &CouplingThresholds::default());
        assert_eq!(
            result.description,
            "0 of 1 source/test pairs below 30% co-change"
        );
    }
}
