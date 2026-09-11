//! One explicit reference drives evidence windows and report calculations.
use barad_dur::{
    backfill, cache,
    cli::{AnalyzeArgs, BackfillArgs, Cli, Commands},
    cmd::analyze,
    collector::Collector,
    config::RepoConfig,
    metrics,
    runner::{self, CollectOptions},
    scorer,
    snapshot::{RepoSnapshot, TimeWindow},
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use clap::Parser;

fn reference() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 10, 0, 0, 1).unwrap()
}

fn args(flags: &[&str]) -> AnalyzeArgs {
    let cli = Cli::parse_from(
        ["barad-dur", "analyze"]
            .into_iter()
            .chain(flags.iter().copied()),
    );
    let Commands::Analyze(args) = cli.command else {
        panic!("analyze command")
    };
    args
}

fn report(snapshot: &RepoSnapshot, at: DateTime<Utc>) -> scorer::AnalysisReport {
    let cfg = RepoConfig::default();
    let god_objects = metrics::health::god_object_files(snapshot, &cfg.thresholds.health);
    let reach = metrics::coupling::growing_coupling_reach(
        snapshot,
        cfg.thresholds.coupling.decay_min_partners,
    );
    let categories = analyze::compute_selected_metrics(
        at,
        snapshot,
        &args(&["--all"]),
        &cfg,
        &god_objects,
        &reach,
    );
    scorer::build_report(
        at,
        snapshot,
        categories,
        None,
        &cfg.weights.as_weight_pairs(),
        &cfg.thresholds,
        &god_objects,
        &reach,
    )
}

fn repository() -> (tempfile::TempDir, Vec<(String, DateTime<Utc>)>) {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    repo.set_head("refs/heads/main").unwrap();
    let points = [
        reference() - Duration::days(182),
        reference() - Duration::days(181),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, timestamp)| {
        let blob = repo
            .blob(format!("pub fn value() -> u32 {{ {i} }}\n").as_bytes())
            .unwrap();
        let mut builder = repo.treebuilder(None).unwrap();
        builder.insert("lib.rs", blob, 0o100644).unwrap();
        let tree_id = builder.write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::new(
            "Alice",
            "alice@example.test",
            &git2::Time::new(timestamp.timestamp(), 0),
        )
        .unwrap();
        let parent = repo.head().ok().map(|head| head.peel_to_commit().unwrap());
        let parents: Vec<_> = parent.iter().collect();
        let id = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "feat: value",
                &tree,
                &parents,
            )
            .unwrap();
        (id.to_string(), timestamp)
    })
    .collect();
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .unwrap();
    (dir, points)
}

#[test]
fn fixed_reference_windows_preserve_default_relative_absolute_and_inclusive_bounds() {
    let at = reference();
    let cfg = RepoConfig::default();
    let window = runner::build_time_window_from_config(at, &cfg, &args(&[]));
    assert_eq!(window.since, Some(at - Duration::days(180)));
    assert_eq!(window.until, Some(at));
    assert_eq!(window.default_months, 6);
    assert!(window.contains(&at));
    assert!(window.contains(&(at - Duration::days(180))));
    assert!(!window.contains(&(at + Duration::seconds(1))));
    assert!(!window.contains(&(at - Duration::days(180) - Duration::seconds(1))));
    let relative = RepoConfig {
        since: Some("3months".into()),
        ..cfg.clone()
    };
    let window = runner::build_time_window_from_config(at, &relative, &args(&[]));
    assert_eq!(window.since, Some(at - Duration::days(90)));
    assert_eq!(window.until, Some(at));
    let window = runner::build_time_window_from_config(at, &cfg, &args(&["--until", "2026-09-01"]));
    assert_eq!(window.since, None);
    assert_eq!(
        window.until,
        Some(Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap())
    );
    let window = runner::build_time_window_from_config(at, &relative, &args(&["--all"]));
    assert_eq!(window.since, None);
    assert_eq!(window.until, None);
}

#[test]
fn fresh_and_cached_reports_share_reference_but_new_invocations_can_advance() {
    let (dir, _) = repository();
    let collector = Collector::open(dir.path(), TimeWindow::full_history()).unwrap();
    let head = collector.head_commit_hash().unwrap();
    let options = CollectOptions {
        show_progress: false,
        verbose: false,
        skip_blame: false,
        no_cache: true,
        cache_only: false,
        cli_exclude_patterns: &[],
        cli_exclude_extensions: &[],
        use_default_excludes: true,
    };
    let fresh = runner::resolve_snapshot(&collector, &head, &options).unwrap();
    let cached = runner::resolve_snapshot(
        &collector,
        &head,
        &CollectOptions {
            no_cache: false,
            cache_only: true,
            ..options
        },
    )
    .unwrap();
    assert_eq!(fresh.created_at, cached.created_at);
    let first = report(&fresh, reference());
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(report(&cached, reference())).unwrap()
    );
    assert_eq!(first.file_ages[0].days_since_modified, 181);
    assert_eq!(first.author_cards[0].days_since_active, 181);
    let code_age = |report: &scorer::AnalysisReport| {
        let metric = report
            .categories
            .iter()
            .flat_map(|category| &category.metrics)
            .find(|metric| metric.name == "Code age")
            .unwrap();
        let metrics::RawValue::Float(months) = metric.raw_value else {
            panic!("measured age")
        };
        months
    };
    assert_eq!(code_age(&first), 181.0 / 30.0);
    let later = report(&cached, reference() + Duration::days(1));
    assert_eq!(later.file_ages[0].days_since_modified, 182);
    assert_eq!(later.author_cards[0].days_since_active, 182);
    assert_eq!(code_age(&later), 182.0 / 30.0);
    assert_eq!(
        later.audit.as_ref().unwrap().dead_files.len(),
        0,
        "two commits exclude dead-file classification"
    );
}

#[test]
fn backfill_persists_selected_commit_timestamps() {
    let (dir, points) = repository();
    backfill::run(
        &BackfillArgs {
            target: dir.path().display().to_string(),
            no_blame: true,
        },
        dir.path(),
    )
    .unwrap();
    let entries = cache::history::load_history(dir.path()).unwrap();
    assert_eq!(entries.len(), points.len());
    for (entry, (head, timestamp)) in entries.iter().zip(points) {
        assert_eq!(entry.head, head);
        assert_eq!(entry.timestamp, timestamp);
        assert_eq!(entry.source.as_deref(), Some("backfill"));
    }
}
