//! File hotspot rows: churn, bug-commit counts, complexity, coupling
//! finding badges, and the per-file churn sparkline.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::config::CouplingThresholds;
use crate::metrics::coupling::all_coupling_findings;
use crate::metrics::file_role::classify;
use crate::snapshot::{CouplingKind, RepoSnapshot};

use super::super::types::HotspotFile;

const BUG_KEYWORDS: &[&str] = &["fix", "bug", "broken", "crash", "regression"];

/// Number of equal time slices the analysis window is split into for the
/// per-file churn sparkline. All files share the same axis so timelines
/// are comparable across rows.
const CHURN_TIMELINE_BUCKETS: usize = 12;

fn churn_timeline(
    commit_ids: Option<&Vec<crate::snapshot::CommitId>>,
    ts_by_id: &HashMap<crate::snapshot::CommitId, i64>,
    min_ts: i64,
    max_ts: i64,
) -> Vec<u32> {
    if min_ts > max_ts {
        return vec![0; CHURN_TIMELINE_BUCKETS]; // snapshot has no commits
    }
    let span = (max_ts - min_ts + 1) as i128;
    commit_ids
        .into_iter()
        .flatten()
        .filter_map(|id| ts_by_id.get(id))
        .fold(vec![0u32; CHURN_TIMELINE_BUCKETS], |mut acc, ts| {
            let idx = ((*ts - min_ts) as i128 * CHURN_TIMELINE_BUCKETS as i128 / span) as usize;
            acc[idx.min(CHURN_TIMELINE_BUCKETS - 1)] += 1;
            acc
        })
}

pub(crate) fn build_hotspots(
    snapshot: &RepoSnapshot,
    coupling: &CouplingThresholds,
) -> Vec<HotspotFile> {
    // Pre-classify bug-fix commits by ID to avoid O(files × commits) message scanning.
    let bug_commit_ids: HashSet<crate::snapshot::CommitId> = snapshot
        .commits
        .iter()
        .filter(|c| {
            let msg = c.message.to_lowercase();
            BUG_KEYWORDS.iter().any(|kw| msg.contains(kw))
        })
        .map(|c| c.id)
        .collect();

    let ts_by_id: HashMap<crate::snapshot::CommitId, i64> = snapshot
        .commits
        .iter()
        .map(|c| (c.id, c.timestamp.timestamp()))
        .collect();
    let (min_ts, max_ts) = ts_by_id
        .values()
        .fold((i64::MAX, i64::MIN), |(lo, hi), &t| (lo.min(t), hi.max(t)));

    let all_findings = all_coupling_findings(snapshot, coupling);
    let finding_counts: HashMap<&Path, (usize, usize, usize, usize)> =
        all_findings.iter().fold(HashMap::new(), |mut acc, f| {
            let entry = acc.entry(f.path.as_path()).or_default();
            match f.kind {
                CouplingKind::Content => entry.0 += 1,
                CouplingKind::Common => entry.1 += 1,
                CouplingKind::Inheritance => entry.2 += 1,
                CouplingKind::Control => entry.3 += 1,
            }
            acc
        });

    let mut files: Vec<HotspotFile> = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary)
        .map(|f| {
            let commit_ids = snapshot.commits_by_file.get(&f.path);
            let churn = commit_ids.map(|v| v.len()).unwrap_or(0);
            let bug_commit_count = commit_ids
                .map(|ids| ids.iter().filter(|id| bug_commit_ids.contains(id)).count())
                .unwrap_or(0);
            let metrics = snapshot
                .file_metrics
                .get(&f.path)
                .cloned()
                .unwrap_or_default();
            let (content_findings, common_findings, inheritance_findings, control_findings) =
                finding_counts
                    .get(f.path.as_path())
                    .copied()
                    .unwrap_or((0, 0, 0, 0));
            HotspotFile {
                path: f.path.to_string_lossy().to_string(),
                role: classify(&f.path),
                churn_count: churn,
                bug_commit_count,
                loc: metrics.loc,
                total_lines: metrics.total_lines,
                cyclomatic_complexity: metrics.cyclomatic_complexity,
                public_methods: metrics.public_methods,
                properties: metrics.properties,
                hotspot_score: 0.0,
                content_findings,
                common_findings,
                control_findings,
                inheritance_findings,
                churn_timeline: churn_timeline(commit_ids, &ts_by_id, min_ts, max_ts),
            }
        })
        .collect();

    if files.is_empty() {
        return files;
    }

    let max_churn = files
        .iter()
        .map(|f| f.churn_count)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_cc = files
        .iter()
        .map(|f| f.cyclomatic_complexity as usize)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_loc = files.iter().map(|f| f.loc).max().unwrap_or(1).max(1);

    for f in &mut files {
        let churn_norm = f.churn_count as f64 / max_churn as f64;
        let cc_norm = f.cyclomatic_complexity as f64 / max_cc as f64;
        let loc_norm = f.loc as f64 / max_loc as f64;
        let base = (churn_norm * 0.5 + cc_norm * 0.3 + loc_norm * 0.2) * 100.0;
        // Content/Common findings multiply risk (severity × change
        // frequency); capped because every consumer assumes 0–100.
        f.hotspot_score = if f.content_findings + f.common_findings > 0 {
            (base * coupling.hotspot_multiplier).min(100.0)
        } else {
            base
        };
    }

    files.sort_by(|a, b| b.hotspot_score.partial_cmp(&a.hotspot_score).unwrap());
    files
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::metrics::file_role::FileRole;
    use crate::snapshot::{CommitId, TimeWindow};
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn hotspot_rows_carry_the_file_role() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![
            make_file_entry("src/lib.rs"),
            make_file_entry("tests/e2e.rs"),
            make_file_entry(".gitlab-ci.yml"),
        ];

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let role_of = |path: &str| {
            hotspots
                .iter()
                .find(|f| f.path == path)
                .map(|f| f.role)
                .unwrap()
        };
        assert_eq!(role_of("src/lib.rs"), FileRole::Source);
        assert_eq!(role_of("tests/e2e.rs"), FileRole::Test);
        assert_eq!(role_of(".gitlab-ci.yml"), FileRole::Config);
    }

    #[test]
    fn churn_timeline_buckets_commits_across_window() {
        use chrono::TimeZone;
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let t_end = Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap();
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit_at(0, t0), make_commit_at(1, t_end)];
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/lib.rs"), vec![CommitId(0), CommitId(1)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let timeline = &hotspots[0].churn_timeline;
        assert_eq!(timeline.len(), 12);
        assert_eq!(timeline[0], 1, "oldest commit lands in the first bucket");
        assert_eq!(timeline[11], 1, "newest commit lands in the last bucket");
        assert_eq!(timeline.iter().sum::<u32>(), 2);
    }

    #[test]
    fn churn_timeline_single_commit_window() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit(0, "feat: only commit")];
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/lib.rs"), vec![CommitId(0)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let timeline = &hotspots[0].churn_timeline;
        assert_eq!(timeline.len(), 12);
        assert_eq!(
            timeline.iter().sum::<u32>(),
            1,
            "zero-span window must not panic or drop the commit"
        );
    }

    #[test]
    fn churn_timeline_is_all_zeros_for_untouched_file() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit(0, "feat: touches nothing tracked")];

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let timeline = &hotspots[0].churn_timeline;
        assert_eq!(timeline.len(), 12);
        assert!(timeline.iter().all(|&v| v == 0));
    }

    #[test]
    fn bug_commit_count_is_zero_when_no_bug_commits() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let path = PathBuf::from("src/lib.rs");
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![
            make_commit(0, "feat: add new endpoint"),
            make_commit(1, "refactor: extract helper"),
        ];
        snapshot
            .commits_by_file
            .insert(path, vec![CommitId(0), CommitId(1)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].bug_commit_count, 0);
    }

    #[test]
    fn bug_commit_count_detects_all_keywords() {
        for (keyword, label) in &[
            ("fix: broken auth", "fix"),
            ("bug in parser found", "bug"),
            ("broken after merge", "broken"),
            ("crash on startup", "crash"),
            ("regression in login", "regression"),
        ] {
            let mut snapshot = RepoSnapshot::new(
                PathBuf::from("/tmp/test"),
                "test".into(),
                "main".into(),
                TimeWindow::default(),
            );
            let path = PathBuf::from("src/lib.rs");
            snapshot.files = vec![make_file_entry("src/lib.rs")];
            snapshot.commits = vec![make_commit(0, keyword)];
            snapshot.commits_by_file.insert(path, vec![CommitId(0)]);

            let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
            assert_eq!(
                hotspots[0].bug_commit_count, 1,
                "keyword '{}' should be detected",
                label
            );
        }
    }

    #[test]
    fn bug_commit_count_is_case_insensitive() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let path = PathBuf::from("src/lib.rs");
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit(0, "FIX: uppercase message")];
        snapshot.commits_by_file.insert(path, vec![CommitId(0)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        assert_eq!(hotspots[0].bug_commit_count, 1);
    }

    #[test]
    fn bug_commit_count_only_counts_commits_touching_that_file() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/a.rs"), make_file_entry("src/b.rs")];
        snapshot.commits = vec![
            make_commit(0, "fix: broken in a"), // bug commit touching a only
            make_commit(1, "feat: add to b"),   // normal commit touching b only
        ];
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/a.rs"), vec![CommitId(0)]);
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/b.rs"), vec![CommitId(1)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let a = hotspots.iter().find(|f| f.path == "src/a.rs").unwrap();
        let b = hotspots.iter().find(|f| f.path == "src/b.rs").unwrap();
        assert_eq!(a.bug_commit_count, 1, "a.rs should have 1 bug commit");
        assert_eq!(b.bug_commit_count, 0, "b.rs should have 0 bug commits");
    }

    #[test]
    fn bug_commit_count_zero_for_file_not_in_commits_by_file() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/new.rs")];
        snapshot.commits = vec![make_commit(0, "fix: something")];
        // commits_by_file intentionally left empty — file not linked to any commit

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        assert_eq!(hotspots[0].bug_commit_count, 0);
    }
    #[test]
    fn hotspot_rows_carry_per_kind_finding_counts() {
        use crate::snapshot::{CouplingFinding, CouplingKind};
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/dirty.rs"),
            crate::metrics::testutil::make_file("src/clean.rs"),
        ];
        snapshot.coupling_findings = vec![
            CouplingFinding {
                path: "src/dirty.rs".into(),
                line: Some(1),
                kind: CouplingKind::Common,
                evidence: "static mut CACHE: usize = 0;".into(),
            },
            CouplingFinding {
                path: "src/dirty.rs".into(),
                line: Some(9),
                kind: CouplingKind::Control,
                evidence: "pub fn go(fast: bool)".into(),
            },
        ];
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let dirty = hotspots.iter().find(|h| h.path == "src/dirty.rs").unwrap();
        assert_eq!(
            (
                dirty.content_findings,
                dirty.common_findings,
                dirty.control_findings
            ),
            (0, 1, 1)
        );
        let clean = hotspots.iter().find(|h| h.path == "src/clean.rs").unwrap();
        assert_eq!(
            (
                clean.content_findings,
                clean.common_findings,
                clean.control_findings
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn hotspot_rows_carry_inheritance_counts_without_score_boost() {
        use crate::snapshot::{CouplingFinding, CouplingKind};
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/deep.ts"),
            crate::metrics::testutil::make_file("src/clean.ts"),
        ];
        snapshot.coupling_findings = vec![CouplingFinding {
            path: "src/deep.ts".into(),
            line: Some(2),
            kind: CouplingKind::Inheritance,
            evidence: "class C extends B → A (depth 2)".into(),
        }];
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let deep = hotspots.iter().find(|h| h.path == "src/deep.ts").unwrap();
        let clean = hotspots.iter().find(|h| h.path == "src/clean.ts").unwrap();
        assert_eq!(deep.inheritance_findings, 1);
        assert_eq!(clean.inheritance_findings, 0);
        // Ladder: only Content/Common multiply hotspot risk. Identical
        // churn/complexity ⇒ identical score despite the inheritance finding.
        assert_eq!(deep.hotspot_score, clean.hotspot_score);
    }

    #[test]
    fn hotspot_content_counts_include_barrel_findings_only_when_toggle_on() {
        // Cross-component import bypassing src/a's barrel — the same shape
        // the gate's ratchet_finding_sets tests use.
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/a/index.ts"),
            crate::metrics::testutil::make_file("src/a/impl.ts"),
            crate::metrics::testutil::make_file("src/b/user.ts"),
        ];
        snapshot
            .import_graph
            .insert("src/b/user.ts".into(), vec!["src/a/impl.ts".into()]);
        let cfg = crate::config::CouplingThresholds::default();
        assert!(cfg.content_barrel_rule, "default toggle must be on");
        let on = build_hotspots(&snapshot, &cfg);
        let user_on = on.iter().find(|h| h.path == "src/b/user.ts").unwrap();
        assert_eq!(
            user_on.content_findings, 1,
            "barrel bypass joins the importing file's content count"
        );

        let cfg_off = crate::config::CouplingThresholds {
            content_barrel_rule: false,
            ..crate::config::CouplingThresholds::default()
        };
        let off = build_hotspots(&snapshot, &cfg_off);
        let user_off = off.iter().find(|h| h.path == "src/b/user.ts").unwrap();
        assert_eq!(
            user_off.content_findings, 0,
            "toggle off must mirror pressman_finding_counts' gating"
        );
    }

    /// Two files identical in churn/CC/LOC; one carries a Common finding.
    /// Base score for both: cc_norm=1, loc_norm=1, churn=0 →
    /// (0.3 + 0.2) × 100 = 50. Flagged file: 50 × 1.25 = 62.5.
    fn twin_snapshot(kind: crate::snapshot::CouplingKind) -> crate::snapshot::RepoSnapshot {
        use crate::snapshot::{CouplingFinding, FileComplexity};
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/flagged.rs"),
            crate::metrics::testutil::make_file("src/plain.rs"),
        ];
        for p in ["src/flagged.rs", "src/plain.rs"] {
            snapshot.file_metrics.insert(
                p.into(),
                FileComplexity {
                    loc: 100,
                    cyclomatic_complexity: 10,
                    ..Default::default()
                },
            );
        }
        snapshot.coupling_findings = vec![CouplingFinding {
            path: "src/flagged.rs".into(),
            line: Some(1),
            kind,
            evidence: "evidence".into(),
        }];
        snapshot
    }

    #[test]
    fn common_finding_multiplies_hotspot_score() {
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Common);
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots
            .iter()
            .find(|h| h.path == "src/flagged.rs")
            .unwrap();
        let plain = hotspots.iter().find(|h| h.path == "src/plain.rs").unwrap();
        assert!((plain.hotspot_score - 50.0).abs() < 1e-9);
        assert!(
            (flagged.hotspot_score - 62.5).abs() < 1e-9,
            "50 × default 1.25 = 62.5, got {}",
            flagged.hotspot_score
        );
    }

    #[test]
    fn control_finding_does_not_multiply_hotspot_score() {
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Control);
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots
            .iter()
            .find(|h| h.path == "src/flagged.rs")
            .unwrap();
        assert!(
            (flagged.hotspot_score - 50.0).abs() < 1e-9,
            "control is the least severe rung — no multiplier (spec)"
        );
    }

    #[test]
    fn multiplied_hotspot_score_caps_at_100() {
        // Base score here is 50 (cc_norm=1, loc_norm=1, churn=0); a large
        // multiplier would push it to 500 — the cap must clamp to 100
        // because every consumer assumes the 0–100 domain.
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Common);
        let cfg = crate::config::CouplingThresholds {
            hotspot_multiplier: 10.0,
            ..crate::config::CouplingThresholds::default()
        };
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots
            .iter()
            .find(|h| h.path == "src/flagged.rs")
            .unwrap();
        assert!(
            (flagged.hotspot_score - 100.0).abs() < 1e-9,
            "consumers assume 0–100; got {}",
            flagged.hotspot_score
        );
    }
}
