use crate::deps::EcosystemReport;
use crate::metrics::{CategoryResult, MetricValue, RawValue};

pub fn compute_deps(ecosystem_reports: &[EcosystemReport]) -> CategoryResult {
    let metrics: Vec<MetricValue> = ecosystem_reports.iter().map(score_ecosystem).collect();
    CategoryResult {
        name: "Dependencies".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

fn score_ecosystem(report: &EcosystemReport) -> MetricValue {
    if report.total_deps == 0 {
        return MetricValue {
            name: format!("{} dependencies", report.ecosystem.display_name()),
            description: "No dependencies found".to_string(),
            raw_value: RawValue::Count(0),
            score: 100,
        };
    }

    let base_score = drift_to_score(report.mean_drift_years);

    // 5-point penalty per HIGH or CRITICAL CVE across all critical deps
    let cve_penalty: u32 = report
        .critical_deps
        .iter()
        .flat_map(|d| &d.vulnerabilities)
        .filter(|v| v.severity == "HIGH" || v.severity == "CRITICAL")
        .count() as u32
        * 5;

    let score = base_score.saturating_sub(cve_penalty);

    let critical_count = report.critical_deps.len();
    let description = if critical_count > 0 {
        format!(
            "{:.1} libyears avg, {} critical callout(s)",
            report.mean_drift_years, critical_count
        )
    } else {
        format!(
            "{:.1} libyears avg, all deps healthy",
            report.mean_drift_years
        )
    };

    MetricValue {
        name: format!("{} dependencies", report.ecosystem.display_name()),
        description,
        raw_value: RawValue::Float(report.mean_drift_years),
        score,
    }
}

fn drift_to_score(mean_drift: f64) -> u32 {
    match mean_drift {
        d if d < 0.5 => 100,
        d if d < 1.0 => 90,
        d if d < 2.0 => 75,
        d if d < 5.0 => 55,
        d if d < 10.0 => 25,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::{DepAge, DepTier, Ecosystem, EcosystemReport, Vuln};

    fn make_report(
        ecosystem: Ecosystem,
        mean_drift: f64,
        critical_deps: Vec<DepAge>,
    ) -> EcosystemReport {
        EcosystemReport {
            ecosystem,
            total_deps: 10,
            mean_drift_years: mean_drift,
            total_drift_years: mean_drift * 10.0,
            critical_deps,
        }
    }

    #[test]
    fn fresh_deps_score_100() {
        let metric = score_ecosystem(&make_report(Ecosystem::Cargo, 0.2, vec![]));
        assert_eq!(metric.score, 100);
    }

    #[test]
    fn one_year_drift_scores_75() {
        let metric = score_ecosystem(&make_report(Ecosystem::Npm, 1.5, vec![]));
        assert_eq!(metric.score, 75);
    }

    #[test]
    fn critical_drift_scores_25() {
        let metric = score_ecosystem(&make_report(Ecosystem::Pip, 7.0, vec![]));
        assert_eq!(metric.score, 25);
    }

    #[test]
    fn cve_penalty_applied() {
        let dep = DepAge {
            name: "vuln-pkg".into(),
            ecosystem: Ecosystem::Npm,
            current_version: "1.0.0".into(),
            drift_years: 0.3,
            tier: DepTier::Fresh,
            vulnerabilities: vec![
                Vuln {
                    id: "CVE-1".into(),
                    severity: "HIGH".into(),
                    description: "".into(),
                },
                Vuln {
                    id: "CVE-2".into(),
                    severity: "CRITICAL".into(),
                    description: "".into(),
                },
            ],
        };
        // base=100, penalty=2*5=10 → 90
        let metric = score_ecosystem(&make_report(Ecosystem::Npm, 0.2, vec![dep]));
        assert_eq!(metric.score, 90);
    }

    #[test]
    fn compute_deps_averages_ecosystems() {
        let reports = vec![
            make_report(Ecosystem::Cargo, 0.2, vec![]), // score 100
            make_report(Ecosystem::Npm, 1.5, vec![]),   // score 75
        ];
        let result = compute_deps(&reports);
        assert_eq!(result.name, "Dependencies");
        assert_eq!(result.score, 87); // (100+75)/2 = 87 (integer division)
        assert_eq!(result.metrics.len(), 2);
    }
}
