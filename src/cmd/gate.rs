use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::analysis::{self, AnalysisInputs, AnalysisResult, CategorySelection};
use crate::cache;
use crate::cli::GateArgs;
use crate::collector::Collector;
use crate::config;
use crate::metrics::coupling::{CouplingEvidence, CouplingFindingCounts};
use crate::metrics::CategoryResult;
use crate::metrics::{coupling, health};
use crate::runner::{self, CollectOptions};
use crate::scorer;
use crate::snapshot::{CouplingFinding, RepoSnapshot, TimeWindow};
use crate::trend::{self, VelocityDirection};

pub fn run_gate(args: GateArgs) -> Result<i32> {
    let reference_time = chrono::Utc::now();
    let local_path = PathBuf::from(&args.target);
    let cfg = config::load(&local_path)?;
    config::validate(&cfg)?;
    let skip_blame = args.skip_blame.unwrap_or(cfg.skip_blame);

    let time_window = TimeWindow::at(reference_time);
    let collector = Collector::open(&local_path, time_window)?;

    let use_default_excludes = cfg.exclude_use_defaults;
    let current_head = collector.head_commit_hash()?;

    let snapshot = runner::resolve_snapshot(
        &collector,
        &current_head,
        &CollectOptions {
            show_progress: false,
            verbose: false,
            skip_blame,
            no_cache: false,
            cache_only: false,
            // `gate` has no CLI exclude flags; exclusions come from
            // `.baraddurignore` and the built-in defaults only.
            cli_exclude_patterns: &[],
            cli_exclude_extensions: &[],
            use_default_excludes,
        },
    )?;

    // Feeds the Health category's "God objects" metric; the gate builds no
    // display report, so nothing else consumes it here.
    let flagged_god_objects = health::god_object_files(&snapshot, &cfg.thresholds.health);

    // Feeds the Coupling category's reach-trend metric.
    let coupling_reach =
        coupling::growing_coupling_reach(&snapshot, cfg.thresholds.coupling.decay_min_partners);

    let weight_pairs = cfg.weights.as_weight_pairs();
    let analysis = analysis::calculate(&AnalysisInputs {
        reference_time,
        snapshot: &snapshot,
        selection: CategorySelection::GATE,
        thresholds: &cfg.thresholds,
        weights: &weight_pairs,
        dependency_evidence: &[],
        god_objects: &flagged_god_objects,
        coupling_reach: &coupling_reach,
    });
    let threshold = args.min_score;
    let score_failed = check_gate_categories(&analysis, &args, threshold);

    let trend_failed = match args.max_decline {
        Some(max_decline) => check_trend_gate_against_history(
            reference_time,
            &local_path,
            &analysis,
            &snapshot,
            &current_head,
            max_decline,
        ),
        None => false,
    };

    let ratchet_failed = if args.no_new_coupling || args.max_new_coupling.is_some() {
        let baseline_ref = args
            .baseline_ref
            .as_deref()
            .expect("clap `requires` guarantees baseline_ref");
        let max_new = args.max_new_coupling.unwrap_or(0);
        let sha = resolve_baseline_ref(&local_path, baseline_ref)?;
        let ignore = crate::collector::BaradDurIgnore::load(&local_path)?;
        let base_snapshot = Collector::collect_snapshot_at_with_ast(
            &local_path,
            &sha,
            &ignore,
            use_default_excludes,
        )?;
        // One derivation per snapshot: the counts (increase summary) and
        // the finding set (new-finding diff) cannot disagree about what
        // "content coupling" means, barrel toggle included.
        let base_evidence = CouplingEvidence::derive(&base_snapshot, &cfg.thresholds.coupling);
        let base_counts = base_evidence
            .finding_counts()
            .unwrap_or(CouplingFindingCounts {
                content: 0,
                common: 0,
                inheritance: 0,
                control: 0,
            });
        let head_counts =
            analysis
                .coupling_evidence
                .finding_counts()
                .unwrap_or(CouplingFindingCounts {
                    content: 0,
                    common: 0,
                    inheritance: 0,
                    control: 0,
                });
        let verdict = ratchet_verdict(
            &base_counts,
            &head_counts,
            &base_evidence.findings,
            &analysis.coupling_evidence.findings,
            max_new,
        );
        println!("{}", print_ratchet(&verdict, baseline_ref, max_new));
        verdict.failed
    } else {
        false
    };

    Ok(gate_exit_code(score_failed, trend_failed, ratchet_failed))
}

/// Resolve a baseline ref (branch, tag, or SHA) to a full commit SHA, in the
/// repo at `repo_path`. Kept separate from `Collector` since it only needs a
/// one-off `revparse` — no snapshot state.
fn resolve_baseline_ref(repo_path: &Path, r: &str) -> anyhow::Result<String> {
    let repo = git2::Repository::discover(repo_path)?;
    let obj = repo.revparse_single(r).map_err(|e| {
        anyhow::anyhow!(
            "cannot resolve baseline ref '{r}': {e}. On CI, shallow clones hide history — \
             set GIT_DEPTH: 0 (GitLab) or fetch the ref first (git fetch origin {r})."
        )
    })?;
    let commit = obj
        .peel_to_commit()
        .map_err(|e| anyhow::anyhow!("baseline ref '{r}' does not point at a commit: {e}"))?;
    Ok(commit.id().to_string())
}

/// Fold the three independent gate checks into a process exit code.
fn gate_exit_code(score_failed: bool, trend_failed: bool, ratchet_failed: bool) -> i32 {
    if score_failed || trend_failed || ratchet_failed {
        1
    } else {
        0
    }
}

/// Render a `RatchetVerdict` as the text `run_gate` prints to stdout.
/// Returns the string (rather than printing directly) so it can be
/// unit-tested against synthetic verdicts without a real git repo.
fn print_ratchet(verdict: &RatchetVerdict, baseline_ref: &str, max_new: usize) -> String {
    if verdict.failed {
        let mut out = format!(
            "RATCHET FAIL: {} new coupling finding(s) vs {} (allowed {})",
            verdict.total_new, baseline_ref, max_new
        );
        for (kind, base, head) in &verdict.increases {
            out.push_str(&format!("\n  {kind}: {base} -> {head}"));
        }
        for f in &verdict.new_findings {
            match f.line {
                Some(l) => {
                    out.push_str(&format!("\n  {}:{} — {}", f.path.display(), l, f.evidence))
                }
                None => out.push_str(&format!("\n  {} — {}", f.path.display(), f.evidence)),
            }
        }
        out
    } else if verdict.total_new == 0 {
        format!("RATCHET PASS: no new coupling findings vs {baseline_ref}")
    } else {
        format!(
            "RATCHET PASS: {} new <= allowed {} vs {}",
            verdict.total_new, max_new, baseline_ref
        )
    }
}

/// Load prior history and decide whether the trend gate fails.
fn check_trend_gate_against_history(
    reference_time: chrono::DateTime<chrono::Utc>,
    local_path: &Path,
    analysis: &AnalysisResult,
    snapshot: &RepoSnapshot,
    current_head: &str,
    max_decline: f64,
) -> bool {
    // Checked, not raw: entries from an older scoring formula are archived
    // rather than compared against. Mixing the two reports a formula change
    // as if the code had moved, failing the gate on every subsequent run.
    let (history, warning) = cache::history::load_history_checked(local_path).unwrap_or_default();
    if let Some(warning) = warning {
        println!("{warning}");
    }
    let current_entry =
        scorer::build_history_entry(reference_time, analysis, snapshot, current_head, None);
    let summary = trend::compute_trend(&history, &snapshot.default_branch, &current_entry);
    check_trend_gate(&summary, max_decline)
}

fn check_trend_gate(summary: &trend::TrendSummary, max_decline: f64) -> bool {
    if summary.delta.is_first {
        println!("TREND: no prior history on this branch — skipping trend check");
        return false;
    }

    if summary.branch_mismatch_warning {
        println!("TREND WARN: prior history is from a different branch");
    }

    match &summary.velocity {
        Some(v) if v.direction == VelocityDirection::Declining => {
            let rate = v.points_per_run.abs();
            if rate > max_decline {
                println!(
                    "FAIL: score declining at {:.1} points/run (limit: {:.1})",
                    rate, max_decline
                );
                true
            } else {
                println!(
                    "PASS: score declining at {:.1} points/run (within limit {:.1})",
                    rate, max_decline
                );
                false
            }
        }
        Some(v) => {
            println!(
                "PASS: score trend {:?} ({:+.1} points/run)",
                v.direction, v.points_per_run
            );
            false
        }
        None => {
            println!("TREND: not enough history to compute velocity");
            false
        }
    }
}

/// Verdict for one score against the threshold: `(failed, line to print)`.
/// An unscored value is not a failure — there is no evidence either way —
/// but it is reported as such, distinct from a category that does not exist.
fn score_verdict(label: &str, score: Option<u32>, threshold: u32) -> (bool, String) {
    match score {
        Some(score) if score < threshold => (
            true,
            format!("FAIL: {label} score {score} < threshold {threshold}"),
        ),
        Some(score) => (
            false,
            format!("PASS: {label} score {score} >= threshold {threshold}"),
        ),
        None => (
            false,
            format!("WARN: {label} has no scored metric (insufficient data), not gated"),
        ),
    }
}

fn find_category<'a>(analysis: &'a AnalysisResult, cat_name: &str) -> Option<&'a CategoryResult> {
    let cat_lower = cat_name.to_lowercase();
    analysis.categories.iter().find(|c| {
        let name_lower = c.name.to_lowercase();
        name_lower == cat_lower || name_lower.contains(&cat_lower)
    })
}

/// `(failed, line)` for a `--category` argument; unknown names never fail.
fn category_verdict(analysis: &AnalysisResult, cat_name: &str, threshold: u32) -> (bool, String) {
    match find_category(analysis, cat_name) {
        Some(cat) => score_verdict(&cat.name, cat.score, threshold),
        None => (
            false,
            format!("WARN: unknown category '{cat_name}', skipping"),
        ),
    }
}

#[cfg(test)]
fn explain_category_gate(analysis: &AnalysisResult, cat_name: &str, threshold: u32) -> String {
    category_verdict(analysis, cat_name, threshold).1
}

fn check_gate_categories(analysis: &AnalysisResult, args: &GateArgs, threshold: u32) -> bool {
    let mut failed = false;

    if args.category.is_empty() {
        let (overall_failed, line) = score_verdict("overall", analysis.overall_score, threshold);
        println!("{line}");
        failed = overall_failed;
    } else {
        for cat_name in &args.category {
            let (cat_failed, line) = category_verdict(analysis, cat_name, threshold);
            println!("{line}");
            failed |= cat_failed;
        }
    }

    failed
}

pub(crate) struct RatchetVerdict {
    pub failed: bool,
    /// (kind label, baseline count, head count) for every kind that increased.
    pub increases: Vec<(&'static str, usize, usize)>,
    /// Findings present at HEAD but not at baseline, identity (path, kind, evidence).
    pub new_findings: Vec<CouplingFinding>,
    pub total_new: usize,
}

pub(crate) fn ratchet_verdict(
    baseline: &CouplingFindingCounts,
    head: &CouplingFindingCounts,
    baseline_findings: &[CouplingFinding],
    head_findings: &[CouplingFinding],
    max_new: usize,
) -> RatchetVerdict {
    // Multiset diff (not a set diff): two findings with identical (path, kind,
    // evidence) in one file are two distinct occurrences, not one. A plain
    // `HashSet` would collapse duplicate-evidence findings into a single key,
    // silently hiding a second identical finding added at HEAD. Baseline
    // key-counts are decremented as head findings are matched against them;
    // once a key's baseline count is exhausted, further occurrences at HEAD
    // are new.
    let key = |f: &CouplingFinding| (f.path.clone(), f.kind, f.evidence.clone());
    let mut base_counts: std::collections::HashMap<_, usize> = std::collections::HashMap::new();
    for f in baseline_findings {
        *base_counts.entry(key(f)).or_insert(0) += 1;
    }
    let new_findings: Vec<CouplingFinding> = head_findings
        .iter()
        .filter(|f| {
            let k = key(f);
            match base_counts.get_mut(&k) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    false
                }
                _ => true,
            }
        })
        .cloned()
        .collect();
    let increases: Vec<(&'static str, usize, usize)> = [
        ("content", baseline.content, head.content),
        ("common", baseline.common, head.common),
        ("inheritance", baseline.inheritance, head.inheritance),
        ("control", baseline.control, head.control),
    ]
    .into_iter()
    .filter(|(_, b, h)| h > b)
    .collect();
    let total_new = new_findings.len();
    RatchetVerdict {
        failed: total_new > max_new,
        increases,
        new_findings,
        total_new,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::CategoryResult;
    use crate::snapshot::CouplingKind;
    use crate::trend::{TrendDelta, TrendSummary, TrendVelocity, VelocityDirection};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_analysis(overall: u32, categories: &[(&str, u32)]) -> AnalysisResult {
        let empty = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        AnalysisResult {
            categories: categories
                .iter()
                .map(|(name, score)| CategoryResult {
                    name: name.to_string(),
                    score: Some(*score),
                    metrics: vec![],
                })
                .collect(),
            overall_score: Some(overall),
            coupling_evidence: crate::metrics::coupling::CouplingEvidence::derive(
                &empty,
                &crate::config::CouplingThresholds::default(),
            ),
        }
    }

    /// The snapshot `make_analysis` results describe: branch `main`, empty.
    fn main_snapshot() -> RepoSnapshot {
        RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        )
    }

    fn make_gate_args(min_score: u32, categories: Vec<String>) -> GateArgs {
        GateArgs {
            target: ".".into(),
            min_score,
            category: categories,
            skip_blame: None,
            max_decline: None,
            no_new_coupling: false,
            max_new_coupling: None,
            baseline_ref: None,
        }
    }

    fn first_summary() -> TrendSummary {
        TrendSummary {
            delta: TrendDelta {
                overall: None,
                delta_vs_oldest: None,
                categories: HashMap::new(),
                is_first: true,
            },
            sparkline: vec![],
            velocity: None,
            branch_mismatch_warning: false,
            history: vec![],
        }
    }

    fn summary_with_velocity(direction: VelocityDirection, points_per_run: f64) -> TrendSummary {
        TrendSummary {
            delta: TrendDelta {
                overall: Some(-5),
                delta_vs_oldest: Some(-5),
                categories: HashMap::new(),
                is_first: false,
            },
            sparkline: vec![],
            velocity: Some(TrendVelocity {
                direction,
                points_per_run,
                window_size: 3,
            }),
            branch_mismatch_warning: false,
            history: vec![],
        }
    }

    // ── check_gate_categories ────────────────────────────────────────

    #[test]
    fn overall_pass() {
        let report = make_analysis(75, &[]);
        let args = make_gate_args(60, vec![]);
        assert!(!check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn overall_fail() {
        let report = make_analysis(50, &[]);
        let args = make_gate_args(60, vec![]);
        assert!(check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn category_pass() {
        let report = make_analysis(80, &[("Health", 75)]);
        let args = make_gate_args(60, vec!["health".into()]);
        assert!(!check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn category_fail() {
        let report = make_analysis(80, &[("Health", 40)]);
        let args = make_gate_args(60, vec!["health".into()]);
        assert!(check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn unscored_category_is_reported_as_not_measurable_and_does_not_fail() {
        let mut report = make_analysis(80, &[("Health", 80)]);
        report.categories.push(CategoryResult {
            name: "Team".into(),
            score: None,
            metrics: vec![],
        });
        let args = make_gate_args(60, vec!["team".into()]);
        assert!(!check_gate_categories(&report, &args, 60));
        assert_eq!(
            explain_category_gate(&report, "team", 60),
            "WARN: Team has no scored metric (insufficient data), not gated"
        );
    }

    #[test]
    fn unscored_overall_is_reported_as_not_measurable_and_does_not_fail() {
        let mut report = make_analysis(80, &[]);
        report.overall_score = None;
        let args = make_gate_args(60, vec![]);
        assert!(!check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn unknown_category_skipped() {
        let report = make_analysis(80, &[("Health", 80)]);
        let args = make_gate_args(60, vec!["nonexistent".into()]);
        assert!(!check_gate_categories(&report, &args, 60));
    }

    // ── run_gate (end-to-end exit code) ──────────────────────────────

    /// A throwaway git repo with one commit, so `run_gate` can analyze it.
    fn temp_git_repo() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@e"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "init"]);
        dir
    }

    #[test]
    fn run_gate_returns_zero_when_threshold_met() {
        let dir = temp_git_repo();
        let mut args = make_gate_args(0, vec![]);
        args.target = dir.path().to_string_lossy().into_owned();
        // Any score >= 0, so the gate passes with exit code 0.
        assert_eq!(run_gate(args).unwrap(), 0);
    }

    #[test]
    fn run_gate_returns_one_when_below_threshold() {
        let dir = temp_git_repo();
        let mut args = make_gate_args(100, vec![]);
        args.target = dir.path().to_string_lossy().into_owned();
        // A trivial repo cannot score 100, so the gate fails with exit code 1.
        assert_eq!(run_gate(args).unwrap(), 1);
    }

    #[test]
    fn run_gate_rejects_invalid_config() {
        let dir = temp_git_repo();
        let cache_dir = dir.path().join(".repository-analysis");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            cache_dir.join("barad-dur.toml"),
            "[thresholds.health]\ngod_node_degree_percentile = 0.0\n",
        )
        .unwrap();
        let mut args = make_gate_args(0, vec![]);
        args.target = dir.path().to_string_lossy().into_owned();
        assert!(
            run_gate(args).is_err(),
            "gate must reject a config that analyze would also reject"
        );
    }

    // ── check_trend_gate_against_history ────────────────────────────────

    fn write_history_entry(dir: &Path, overall_score: u32, schema_version: u32) {
        let cache_dir = dir.join(crate::cache::storage::CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        let entry = serde_json::json!({
            "timestamp": "2026-01-01T00:00:00Z",
            "head": "abc123",
            "overall_score": overall_score,
            "category_scores": {},
            "metrics": {},
            "counts": {"commits": 1, "files": 2, "authors": 3},
            "branch": "main",
            "schema_version": schema_version,
        });
        std::fs::write(cache_dir.join("trends.json"), format!("{entry}\n")).unwrap();
    }

    #[test]
    fn stale_scoring_history_does_not_trip_the_trend_gate() {
        // Entries written by an older scoring formula are a stale computation,
        // not history. `analyze` and `backfill` archive them; the gate must
        // too. Otherwise CI fails on a decline that is purely the formula
        // changing underneath it — and keeps failing every run, because
        // nothing on the gate path ever sets the old file aside.
        let dir = TempDir::new().unwrap();
        write_history_entry(dir.path(), 95, crate::scorer::HISTORY_SCHEMA_VERSION - 1);

        let report = make_analysis(40, &[]);
        let failed = check_trend_gate_against_history(
            chrono::Utc::now(),
            dir.path(),
            &report,
            &main_snapshot(),
            "deadbeef",
            2.0,
        );

        assert!(
            !failed,
            "a 95 -> 40 drop across a scoring-formula change is not a decline in the code"
        );
        assert!(
            dir.path()
                .join(crate::cache::storage::CACHE_DIR)
                .join("trends.json.bak")
                .exists(),
            "the stale history must be archived so the next run starts clean"
        );
    }

    #[test]
    fn current_version_history_still_trips_the_trend_gate() {
        // The guard above must not neuter the gate: a real decline measured
        // by the same formula still has to fail.
        let dir = TempDir::new().unwrap();
        write_history_entry(dir.path(), 95, crate::scorer::HISTORY_SCHEMA_VERSION);

        let report = make_analysis(40, &[]);
        let failed = check_trend_gate_against_history(
            chrono::Utc::now(),
            dir.path(),
            &report,
            &main_snapshot(),
            "deadbeef",
            2.0,
        );

        assert!(
            failed,
            "a 95 -> 40 drop under one formula is a real decline"
        );
    }

    // ── check_trend_gate ────────────────────────────────────────────

    #[test]
    fn trend_first_run_passes() {
        assert!(!check_trend_gate(&first_summary(), 2.0));
    }

    #[test]
    fn trend_declining_above_limit_fails() {
        let s = summary_with_velocity(VelocityDirection::Declining, -5.0);
        assert!(check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_declining_below_limit_passes() {
        let s = summary_with_velocity(VelocityDirection::Declining, -1.0);
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_improving_passes() {
        let s = summary_with_velocity(VelocityDirection::Improving, 3.0);
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_stable_passes() {
        let s = summary_with_velocity(VelocityDirection::Stable, 0.1);
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_no_velocity_passes() {
        let mut s = first_summary();
        s.delta.is_first = false; // not first, but no velocity computed
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_branch_mismatch_warning_does_not_fail() {
        let mut s = summary_with_velocity(VelocityDirection::Stable, 0.1);
        s.branch_mismatch_warning = true;
        assert!(!check_trend_gate(&s, 2.0));
    }

    // ── ratchet_verdict ────────────────────────────────────────────

    fn finding(path: &str, kind: CouplingKind, evidence: &str) -> CouplingFinding {
        CouplingFinding {
            path: path.into(),
            line: Some(1),
            kind,
            evidence: evidence.into(),
        }
    }

    #[test]
    fn ratchet_passes_when_head_equals_baseline() {
        let f = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let c = CouplingFindingCounts {
            content: 0,
            common: 1,
            inheritance: 0,
            control: 0,
        };
        let v = ratchet_verdict(&c, &c, &f, &f, 0);
        assert!(!v.failed);
        assert_eq!(v.total_new, 0);
        assert!(v.increases.is_empty());
    }

    #[test]
    fn ratchet_fails_on_one_new_finding_and_names_it() {
        let base = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let head = vec![
            finding("a.rs", CouplingKind::Common, "static mut X"),
            finding("b.rs", CouplingKind::Content, "#[path = \"../x.rs\"]"),
        ];
        let cb = CouplingFindingCounts {
            content: 0,
            common: 1,
            inheritance: 0,
            control: 0,
        };
        let ch = CouplingFindingCounts {
            content: 1,
            common: 1,
            inheritance: 0,
            control: 0,
        };
        let v = ratchet_verdict(&cb, &ch, &base, &head, 0);
        assert!(v.failed);
        assert_eq!(v.total_new, 1);
        assert_eq!(v.new_findings[0].path, std::path::PathBuf::from("b.rs"));
        assert_eq!(v.increases, vec![("content", 0, 1)]);
    }

    #[test]
    fn ratchet_allowance_admits_exactly_n() {
        let base: Vec<CouplingFinding> = vec![];
        let head = vec![
            finding("a.rs", CouplingKind::Control, "pub fn f(flag: bool)"),
            finding("b.rs", CouplingKind::Control, "pub fn g(flag: bool)"),
        ];
        let cb = CouplingFindingCounts {
            content: 0,
            common: 0,
            inheritance: 0,
            control: 0,
        };
        let ch = CouplingFindingCounts {
            content: 0,
            common: 0,
            inheritance: 0,
            control: 2,
        };
        assert!(
            !ratchet_verdict(&cb, &ch, &base, &head, 2).failed,
            "n == allowance passes"
        );
        assert!(
            ratchet_verdict(&cb, &ch, &base, &head, 1).failed,
            "n > allowance fails"
        );
    }

    #[test]
    fn ratchet_ignores_line_number_shifts() {
        let mut moved = finding("a.rs", CouplingKind::Common, "static mut X");
        moved.line = Some(99);
        let base = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let c = CouplingFindingCounts {
            content: 0,
            common: 1,
            inheritance: 0,
            control: 0,
        };
        let v = ratchet_verdict(&c, &c, &base, &[moved], 0);
        assert!(!v.failed, "same finding at a different line is not new");
    }

    #[test]
    fn ratchet_removed_findings_do_not_mask_new_ones() {
        // one removed + one added elsewhere: counts flat, set diff catches it
        let base = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let head = vec![finding("b.rs", CouplingKind::Common, "static mut Y")];
        let c = CouplingFindingCounts {
            content: 0,
            common: 1,
            inheritance: 0,
            control: 0,
        };
        let v = ratchet_verdict(&c, &c, &base, &head, 0);
        assert!(
            v.failed,
            "moved-plus-renamed global is a new finding even with flat counts"
        );
        assert_eq!(v.total_new, 1);
    }

    #[test]
    fn ratchet_catches_duplicate_evidence_findings() {
        // Two findings with identical (path, kind, evidence) must not collapse:
        // adding a second identical global write is a new finding.
        let base = vec![finding(
            "a.js",
            CouplingKind::Common,
            "globalThis.ready = true;",
        )];
        let head = vec![
            finding("a.js", CouplingKind::Common, "globalThis.ready = true;"),
            finding("a.js", CouplingKind::Common, "globalThis.ready = true;"),
        ];
        let cb = CouplingFindingCounts {
            content: 0,
            common: 1,
            inheritance: 0,
            control: 0,
        };
        let ch = CouplingFindingCounts {
            content: 0,
            common: 2,
            inheritance: 0,
            control: 0,
        };
        let v = ratchet_verdict(&cb, &ch, &base, &head, 0);
        assert!(
            v.failed,
            "duplicate-evidence second finding must fail the ratchet"
        );
        assert_eq!(v.total_new, 1);
        assert_eq!(v.new_findings.len(), 1);
    }

    #[test]
    fn ratchet_reports_inheritance_increase() {
        let baseline = CouplingFindingCounts {
            content: 0,
            common: 0,
            inheritance: 1,
            control: 0,
        };
        let head = CouplingFindingCounts {
            content: 0,
            common: 0,
            inheritance: 3,
            control: 0,
        };
        let verdict = ratchet_verdict(&baseline, &head, &[], &[], 0);
        assert!(verdict.increases.contains(&("inheritance", 1, 3)));
    }

    // ── print_ratchet ────────────────────────────────────────────────

    #[test]
    fn print_ratchet_fail_lists_increases_and_new_findings() {
        let verdict = RatchetVerdict {
            failed: true,
            increases: vec![("content", 0, 1)],
            new_findings: vec![finding(
                "b.rs",
                CouplingKind::Content,
                "#[path = \"../x.rs\"]",
            )],
            total_new: 1,
        };
        let out = print_ratchet(&verdict, "main", 0);
        assert_eq!(
            out,
            "RATCHET FAIL: 1 new coupling finding(s) vs main (allowed 0)\n\
             \x20\x20content: 0 -> 1\n\
             \x20\x20b.rs:1 — #[path = \"../x.rs\"]"
        );
    }

    #[test]
    fn print_ratchet_fail_omits_line_when_none() {
        let mut barrel_finding = finding("b/index.ts", CouplingKind::Content, "barrel bypass");
        barrel_finding.line = None;
        let verdict = RatchetVerdict {
            failed: true,
            increases: vec![("content", 0, 1)],
            new_findings: vec![barrel_finding],
            total_new: 1,
        };
        let out = print_ratchet(&verdict, "main", 0);
        assert!(
            out.contains("\n  b/index.ts — barrel bypass"),
            "expected no `:line` when line is None, got: {out}"
        );
    }

    #[test]
    fn print_ratchet_pass_no_new_findings() {
        let verdict = RatchetVerdict {
            failed: false,
            increases: vec![],
            new_findings: vec![],
            total_new: 0,
        };
        let out = print_ratchet(&verdict, "main", 0);
        assert_eq!(out, "RATCHET PASS: no new coupling findings vs main");
    }

    #[test]
    fn print_ratchet_pass_within_allowance() {
        let verdict = RatchetVerdict {
            failed: false,
            increases: vec![("control", 0, 1)],
            new_findings: vec![finding(
                "c.rs",
                CouplingKind::Control,
                "pub fn f(flag: bool)",
            )],
            total_new: 1,
        };
        let out = print_ratchet(&verdict, "main", 2);
        assert_eq!(out, "RATCHET PASS: 1 new <= allowed 2 vs main");
    }

    // ── gate_exit_code ───────────────────────────────────────────────

    #[test]
    fn gate_exit_code_zero_when_all_pass() {
        assert_eq!(gate_exit_code(false, false, false), 0);
    }

    #[test]
    fn gate_exit_code_one_when_score_fails() {
        assert_eq!(gate_exit_code(true, false, false), 1);
    }

    #[test]
    fn gate_exit_code_one_when_trend_fails() {
        assert_eq!(gate_exit_code(false, true, false), 1);
    }

    #[test]
    fn gate_exit_code_one_when_ratchet_fails() {
        assert_eq!(gate_exit_code(false, false, true), 1);
    }

    #[test]
    fn gate_exit_code_one_when_all_fail() {
        assert_eq!(gate_exit_code(true, true, true), 1);
    }
}
