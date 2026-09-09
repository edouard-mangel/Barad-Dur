use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use ts_rs::{Config, TS};

use crate::deps::{DepAge, DepTier, Ecosystem, EcosystemReport, Vuln};
use crate::metrics::file_role::FileRole;
use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::scorer::{
    ActionItem, AnalysisReport, AuditReport, AuthorCard, AuthorShare, CallGraphReport, ChurnBucket,
    ChurnTimelineReport, CouplingFindingCounts, CouplingPair, CouplingTrend, CrisisFile, DeadFile,
    DirConcentration, EntityTrendDirection, FileAge, FileCouplingMetrics, FileOwnership,
    FunctionHub, HistoryCounts, HistoryEntry, HotspotFile, ImportEdge, LongMethodThresholds,
    RemoteMeta, ScoreThresholds, VelocityBucket, HISTORY_SCHEMA_VERSION,
};

pub fn export_report_types(output: &Path) -> Result<()> {
    let generated =
        tempfile::tempdir().context("failed to create contract generation directory")?;
    generate_report_types(generated.path())?;
    let declarations = declaration_files(generated.path())?;

    fs::create_dir_all(output).with_context(|| format!("failed to create {}", output.display()))?;
    declaration_files(output)?.keys().try_for_each(|name| {
        fs::remove_file(output.join(name))
            .with_context(|| format!("failed to remove {}", output.join(name).display()))
    })?;
    declarations.into_iter().try_for_each(|(name, contents)| {
        fs::write(output.join(&name), contents)
            .with_context(|| format!("failed to write {}", output.join(name).display()))
    })
}

fn generate_report_types(output: &Path) -> Result<()> {
    let config = Config::new().with_out_dir(output).with_large_int("number");
    AnalysisReport::export_all(&config)
        .context("failed to export report TypeScript declarations")?;
    normalize_generated_declarations(output)
}

fn normalize_generated_declarations(output: &Path) -> Result<()> {
    declaration_files(output)?
        .keys()
        .try_for_each(|name| -> Result<()> {
            let path = output.join(name);
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let normalized = contents
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(&path, format!("{normalized}\n"))
                .with_context(|| format!("failed to write {}", path.display()))
        })
}

pub fn check_report_types(committed: &Path) -> Result<()> {
    let generated = tempfile::tempdir().context("failed to create contract check directory")?;
    generate_report_types(generated.path())?;

    let expected = declaration_files(generated.path())?;
    let actual = declaration_files(committed)?;
    let expected_names = expected.keys().cloned().collect::<BTreeSet<_>>();
    let actual_names = actual.keys().cloned().collect::<BTreeSet<_>>();

    let edited = expected_names
        .intersection(&actual_names)
        .filter(|name| expected.get(*name) != actual.get(*name))
        .cloned()
        .collect::<Vec<_>>();
    let missing = expected_names
        .difference(&actual_names)
        .cloned()
        .collect::<Vec<_>>();
    let unexpected = actual_names
        .difference(&expected_names)
        .cloned()
        .collect::<Vec<_>>();

    if edited.is_empty() && missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }

    let details = [
        ("edited", edited),
        ("missing", missing),
        ("unexpected", unexpected),
    ]
    .into_iter()
    .filter(|(_, files)| !files.is_empty())
    .map(|(kind, files)| {
        let files = files
            .iter()
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("{kind}: {files}")
    })
    .collect::<Vec<_>>()
    .join("\n");
    bail!("generated report type declarations have drifted\n{details}")
}

pub fn report_fixture_json() -> Result<String> {
    let serialized = serde_json::to_string_pretty(&report_fixture())?;
    Ok(format!("{serialized}\n"))
}

pub fn report_fixture_typescript() -> Result<String> {
    let json = report_fixture_json()?;
    Ok(format!(
        "import type {{ AnalysisReport }} from \"../../../dashboard/src/report/generated/AnalysisReport\"\n\nconst currentReport: AnalysisReport = {json}\nexport default currentReport\n"
    ))
}

pub fn export_report_fixture(output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(output, report_fixture_typescript()?)
        .with_context(|| format!("failed to write {}", output.display()))
}

fn report_fixture() -> AnalysisReport {
    let timestamp = "2025-01-02T03:04:05Z".parse().expect("fixed timestamp");
    let metrics = vec![
        fixture_metric("integer", RawValue::Integer(-3), Some(90)),
        fixture_metric("float", RawValue::Float(1.25), Some(80)),
        fixture_metric("percentage", RawValue::Percentage(125.5), Some(70)),
        fixture_metric("count", RawValue::Count(4), Some(60)),
        fixture_metric("text", RawValue::Text("N/A".into()), None),
        fixture_metric(
            "list",
            RawValue::List(vec!["a.rs".into(), "b.rs".into()]),
            Some(50),
        ),
    ];
    let category = CategoryResult {
        name: "Health".into(),
        score: Some(70),
        metrics,
    };

    AnalysisReport {
        repo_name: "contract-fixture".into(),
        branch: "main".into(),
        time_window_months: 6,
        total_commits: 12,
        total_authors: 2,
        total_files: 3,
        overall_score: None,
        categories: vec![category],
        top_actions: vec![
            ActionItem {
                text: "[Health] integer (score: 90) — Inspect the hotspot".into(),
                target_tab: Some("hotspots".into()),
                sort_by: Some("complexity".into()),
            },
            ActionItem {
                text: "[Health] list (score: 50) — Split responsibilities".into(),
                target_tab: None,
                sort_by: None,
            },
        ],
        coupling_actions: vec![ActionItem {
            text: "Reduce cross-boundary coupling".into(),
            target_tab: Some("coupling".into()),
            sort_by: None,
        }],
        remote_meta: Some(RemoteMeta {
            url: "https://example.test/owner/repo".into(),
            stars: Some(42),
            description: None,
            language: Some("Rust".into()),
            open_issues: None,
        }),
        file_hotspots: vec![HotspotFile {
            path: "src/lib.rs".into(),
            role: FileRole::Source,
            churn_count: 7,
            bug_commit_count: 2,
            loc: 120,
            total_lines: 150,
            cyclomatic_complexity: 11,
            public_methods: 4,
            properties: 3,
            hotspot_score: 87.5,
            coupling_trend: Some(CouplingTrend {
                first_half_partners: 2,
                second_half_partners: 5,
            }),
            content_findings: 1,
            common_findings: 2,
            control_findings: 3,
            inheritance_findings: 4,
            churn_timeline: vec![0, 2, 1],
            complexity_trend: Some(EntityTrendDirection::Growing),
            churn_trend: Some(EntityTrendDirection::Stable),
        }],
        coupling_pairs: vec![CouplingPair {
            file_a: "src/lib.rs".into(),
            file_b: "tests/lib_test.rs".into(),
            co_changes: 5,
            coupling_pct: 62.5,
            cross_boundary: true,
            is_test_pair: true,
            growth_a: 10,
            growth_b: -2,
            coupling_trend: Some(EntityTrendDirection::Shrinking),
        }],
        author_ownership: vec![FileOwnership {
            path: "src/lib.rs".into(),
            authors: vec![AuthorShare {
                name: "Ada".into(),
                pct: 75.0,
            }],
        }],
        file_ages: vec![FileAge {
            path: "src/lib.rs".into(),
            last_modified: timestamp,
            days_since_modified: 9,
        }],
        author_cards: vec![AuthorCard {
            name: "Ada".into(),
            email: "ada@example.test".into(),
            commit_count: 8,
            files_owned: 1,
            lines_owned: 90,
            avg_commit_quality: 0.75,
            top_files: vec!["src/lib.rs".into()],
            last_active: timestamp,
            days_since_active: 9,
            directories_touched: 2,
        }],
        history: vec![HistoryEntry {
            timestamp,
            head: "0123456789abcdef".into(),
            overall_score: Some(70),
            categories: [("Health".into(), Some(70))].into(),
            metrics: [("integer".into(), 90)].into(),
            counts: HistoryCounts {
                commits: 12,
                files: 3,
                authors: 2,
                content_coupling: Some(1),
                common_coupling: None,
                control_coupling: Some(3),
                inheritance_coupling: None,
            },
            branch: "main".into(),
            schema_version: HISTORY_SCHEMA_VERSION,
            source: Some("fixture".into()),
        }],
        dep_ecosystem_reports: vec![EcosystemReport {
            ecosystem: Ecosystem::Cargo,
            total_deps: 1,
            mean_drift_years: 2.5,
            total_drift_years: 2.5,
            critical_deps: vec![DepAge {
                name: "example".into(),
                ecosystem: Ecosystem::Cargo,
                current_version: "1.0.0".into(),
                drift_years: 2.5,
                tier: DepTier::Stale,
                vulnerabilities: vec![Vuln {
                    id: "CVE-2025-0001".into(),
                    severity: "high".into(),
                    description: "fixture vulnerability".into(),
                }],
            }],
        }],
        audit: Some(AuditReport {
            crisis_files: vec![CrisisFile {
                path: "src/lib.rs".into(),
                crisis_commit_count: 1,
                total_commit_count: 7,
                crisis_ratio: 1.0 / 7.0,
            }],
            dir_concentration: vec![DirConcentration {
                dir: "src".into(),
                file_count: 2,
                loc: 200,
                pct_of_total: 80.0,
            }],
            dead_files: vec![DeadFile {
                path: "src/old.rs".into(),
                days_since_modified: 730,
                churn_count: 1,
            }],
            velocity_buckets: vec![VelocityBucket {
                week_start: "2024-12-30".into(),
                commit_count: 4,
                author_count: 2,
            }],
        }),
        per_file_coupling: vec![FileCouplingMetrics {
            path: "src/lib.rs".into(),
            ca: 2,
            ce: 3,
            instability: 0.6,
        }],
        import_edges: vec![ImportEdge {
            from: "src/main.rs".into(),
            to: "src/lib.rs".into(),
        }],
        import_cycles: vec![vec!["src/a.rs".into(), "src/b.rs".into()]],
        coupling_finding_counts: Some(CouplingFindingCounts {
            content: 1,
            common: 2,
            inheritance: 4,
            control: 3,
        }),
        call_graph: Some(CallGraphReport {
            resolution_rate: 0.8,
            edges_resolved: 8,
            edges_same_file: 1,
            edges_unresolved: 2,
            call_resolution_floor: 0.5,
            function_hubs: vec![FunctionHub {
                path: "src/lib.rs".into(),
                name: "run".into(),
                resolved_in_degree: 4,
            }],
        }),
        churn_timeline: Some(ChurnTimelineReport {
            bucket_days: 1,
            merge_commits_excluded: true,
            buckets: vec![ChurnBucket {
                date: "2025-01-02".into(),
                added: 12,
                deleted: 3,
            }],
        }),
        score_thresholds: ScoreThresholds {
            good_min: 71,
            warn_min: 41,
        },
        long_method_thresholds: LongMethodThresholds {
            cc: 10,
            cc_floor: 3,
            loc: 50,
            ui_loc: 100,
        },
    }
}

fn fixture_metric(name: &str, raw_value: RawValue, score: Option<u32>) -> MetricValue {
    MetricValue {
        name: name.into(),
        description: format!("{name} fixture"),
        raw_value,
        score,
    }
}

fn declaration_files(directory: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
        .filter_map(|entry| match entry {
            Ok(entry) if entry.path().extension().is_some_and(|ext| ext == "ts") => {
                Some((entry.file_name().into(), fs::read(entry.path())))
            }
            Ok(_) => None,
            Err(error) => Some((PathBuf::new(), Err(error))),
        })
        .map(|(name, contents)| {
            contents
                .with_context(|| format!("failed to read {}", directory.join(&name).display()))
                .map(|contents| (name, contents))
        })
        .collect()
}
