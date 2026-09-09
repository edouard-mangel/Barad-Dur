#![cfg(feature = "export-types")]

use std::fs;

use barad_dur::report_contract::{check_report_types, export_report_types};

#[test]
fn export_covers_tagged_values_lowercase_roles_and_nullable_scores() {
    let output = tempfile::tempdir().unwrap();

    export_report_types(output.path()).unwrap();

    let raw_value = fs::read_to_string(output.path().join("RawValue.ts")).unwrap();
    assert!(raw_value.contains("{ \"Integer\": number }"));
    assert!(raw_value.contains("{ \"Float\": number }"));
    assert!(raw_value.contains("{ \"Percentage\": number }"));
    assert!(raw_value.contains("{ \"Count\": number }"));
    assert!(raw_value.contains("{ \"Text\": string }"));
    assert!(raw_value.contains("{ \"List\": Array<string> }"));

    let file_role = fs::read_to_string(output.path().join("FileRole.ts")).unwrap();
    assert!(file_role.contains("\"source\""));
    assert!(file_role.contains("\"test\""));

    let metric = fs::read_to_string(output.path().join("MetricValue.ts")).unwrap();
    assert!(metric.contains("score: number | null"));

    let report = fs::read_to_string(output.path().join("AnalysisReport.ts")).unwrap();
    assert!(report.contains("overall_score: number | null"));
    assert!(report.contains("coupling_finding_counts?: CouplingFindingCounts"));
    assert!(report.contains("call_graph?: CallGraphReport"));
    assert!(report.contains("churn_timeline?: ChurnTimelineReport"));

    let history_counts = fs::read_to_string(output.path().join("HistoryCounts.ts")).unwrap();
    assert!(history_counts.contains("content_coupling?: number,"));
    assert!(!history_counts.contains("content_coupling?: number | null"));

    directory_snapshot(output.path())
        .into_iter()
        .for_each(|(name, contents)| {
            let contents = String::from_utf8(contents).unwrap();
            assert!(
                contents.lines().all(|line| line.trim_end() == line),
                "{name} contains trailing whitespace"
            );
        });
}

#[test]
fn check_accepts_an_identical_complete_export_without_rewriting_it() {
    let committed = tempfile::tempdir().unwrap();
    export_report_types(committed.path()).unwrap();
    let before = directory_snapshot(committed.path());

    check_report_types(committed.path()).unwrap();

    assert_eq!(directory_snapshot(committed.path()), before);
}

#[test]
fn export_replaces_the_complete_declaration_set() {
    let output = tempfile::tempdir().unwrap();
    export_report_types(output.path()).unwrap();
    fs::write(output.path().join("Obsolete.ts"), "obsolete\n").unwrap();

    export_report_types(output.path()).unwrap();

    assert!(!output.path().join("Obsolete.ts").exists());
    check_report_types(output.path()).unwrap();
}

#[test]
fn check_reports_edited_missing_and_unexpected_declarations() {
    let committed = tempfile::tempdir().unwrap();
    export_report_types(committed.path()).unwrap();

    fs::write(committed.path().join("AnalysisReport.ts"), "edited\n").unwrap();
    fs::remove_file(committed.path().join("RawValue.ts")).unwrap();
    fs::write(committed.path().join("Obsolete.ts"), "obsolete\n").unwrap();

    let error = check_report_types(committed.path())
        .unwrap_err()
        .to_string();

    assert!(error.contains("edited: AnalysisReport.ts"), "{error}");
    assert!(error.contains("missing: RawValue.ts"), "{error}");
    assert!(error.contains("unexpected: Obsolete.ts"), "{error}");
}

fn directory_snapshot(path: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut files = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}
