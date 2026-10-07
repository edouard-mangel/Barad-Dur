use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub enum Ecosystem {
    Cargo,
    Npm,
    Pip,
    Nuget,
}

impl Ecosystem {
    pub fn osv_name(&self) -> &'static str {
        match self {
            Ecosystem::Cargo => "crates.io",
            Ecosystem::Npm => "npm",
            Ecosystem::Pip => "PyPI",
            Ecosystem::Nuget => "NuGet",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Ecosystem::Cargo => "Cargo",
            Ecosystem::Npm => "npm",
            Ecosystem::Pip => "pip",
            Ecosystem::Nuget => "NuGet",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub enum DepTier {
    Fresh,    // drift < 0.5y
    Aging,    // 0.5y – 2y
    Stale,    // 2y – 5y
    Critical, // > 5y
}

impl DepTier {
    pub fn from_drift(drift_years: f64) -> Self {
        match drift_years {
            d if d < 0.5 => DepTier::Fresh,
            d if d < 2.0 => DepTier::Aging,
            d if d < 5.0 => DepTier::Stale,
            _ => DepTier::Critical,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct Vuln {
    pub id: String,
    pub severity: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct DepAge {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub current_version: String,
    pub drift_years: f64,
    pub tier: DepTier,
    pub vulnerabilities: Vec<Vuln>,
}

impl DepAge {
    pub fn is_critical_callout(&self) -> bool {
        self.tier == DepTier::Critical || !self.vulnerabilities.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct EcosystemReport {
    pub ecosystem: Ecosystem,
    pub total_deps: usize,
    pub mean_drift_years: f64,
    pub total_drift_years: f64,
    pub critical_deps: Vec<DepAge>,
}

/// Group resolved dependencies into one report per ecosystem: how many there
/// are, how far behind they run on average, and which need calling out.
/// Reports are ordered by ecosystem display name, ignoring case, so the same
/// dependencies always produce the same report.
pub fn build_ecosystem_reports(dep_ages: Vec<DepAge>) -> Vec<EcosystemReport> {
    use std::collections::HashMap;

    let mut by_ecosystem: HashMap<String, Vec<DepAge>> = HashMap::new();
    for dep in dep_ages {
        by_ecosystem
            .entry(dep.ecosystem.display_name().to_string())
            .or_default()
            .push(dep);
    }

    let mut groups: Vec<(String, Vec<DepAge>)> = by_ecosystem.into_iter().collect();
    groups.sort_by_key(|(name, _)| name.to_lowercase());

    groups
        .into_iter()
        .map(|(_, deps)| {
            let total = deps.len();
            let total_drift: f64 = deps.iter().map(|d| d.drift_years).sum();
            // Every group holds at least the dependency that created it.
            let mean_drift = total_drift / total as f64;
            let critical_deps: Vec<DepAge> = deps
                .iter()
                .filter(|d| d.is_critical_callout())
                .cloned()
                .collect();
            let ecosystem = deps[0].ecosystem.clone();
            EcosystemReport {
                ecosystem,
                total_deps: total,
                mean_drift_years: mean_drift,
                total_drift_years: total_drift,
                critical_deps,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep(name: &str, ecosystem: Ecosystem, drift_years: f64, tier: DepTier) -> DepAge {
        DepAge {
            name: name.to_string(),
            ecosystem,
            current_version: "1.0.0".to_string(),
            drift_years,
            tier,
            vulnerabilities: Vec::new(),
        }
    }

    fn vulnerable(mut dep: DepAge) -> DepAge {
        dep.vulnerabilities.push(Vuln {
            id: "OSV-1".to_string(),
            severity: "HIGH".to_string(),
            description: "d".to_string(),
        });
        dep
    }

    fn names(deps: &[DepAge]) -> Vec<&str> {
        deps.iter().map(|d| d.name.as_str()).collect()
    }

    #[test]
    fn no_dependencies_give_no_reports() {
        assert!(build_ecosystem_reports(Vec::new()).is_empty());
    }

    #[test]
    fn reports_total_and_mean_drift_per_ecosystem() {
        let reports = build_ecosystem_reports(vec![
            dep("a", Ecosystem::Cargo, 1.0, DepTier::Aging),
            dep("b", Ecosystem::Cargo, 3.0, DepTier::Stale),
            dep("c", Ecosystem::Npm, 0.25, DepTier::Fresh),
        ]);
        assert_eq!(reports.len(), 2);

        let cargo = &reports[0];
        assert_eq!(cargo.ecosystem, Ecosystem::Cargo);
        assert_eq!(cargo.total_deps, 2);
        assert_eq!(cargo.total_drift_years, 4.0); // 1.0 + 3.0
        assert_eq!(cargo.mean_drift_years, 2.0); // 4.0 / 2

        let npm = &reports[1];
        assert_eq!(npm.ecosystem, Ecosystem::Npm);
        assert_eq!(npm.total_deps, 1);
        assert_eq!(npm.total_drift_years, 0.25);
        assert_eq!(npm.mean_drift_years, 0.25);
    }

    #[test]
    fn critical_callouts_are_critical_or_vulnerable_dependencies_in_input_order() {
        let reports = build_ecosystem_reports(vec![
            dep("old", Ecosystem::Cargo, 6.0, DepTier::Critical),
            dep("fine", Ecosystem::Cargo, 0.1, DepTier::Fresh),
            vulnerable(dep("hole", Ecosystem::Cargo, 0.1, DepTier::Fresh)),
            dep("stale", Ecosystem::Cargo, 3.0, DepTier::Stale),
        ]);
        assert_eq!(names(&reports[0].critical_deps), ["old", "hole"]);
    }

    #[test]
    fn ecosystems_come_out_in_a_fixed_order_whatever_the_input_order() {
        // Grouping goes through a hash map, whose iteration order changes from
        // run to run; the report must not inherit that. Order is by display
        // name, ignoring case.
        let input = || {
            vec![
                dep("p", Ecosystem::Pip, 1.0, DepTier::Aging),
                dep("n", Ecosystem::Nuget, 1.0, DepTier::Aging),
                dep("m", Ecosystem::Npm, 1.0, DepTier::Aging),
                dep("c", Ecosystem::Cargo, 1.0, DepTier::Aging),
            ]
        };
        for _ in 0..25 {
            let order: Vec<&str> = build_ecosystem_reports(input())
                .iter()
                .map(|r| r.ecosystem.display_name())
                .collect();
            assert_eq!(order, ["Cargo", "npm", "NuGet", "pip"]);
        }
        let mut reversed = input();
        reversed.reverse();
        let order: Vec<&str> = build_ecosystem_reports(reversed)
            .iter()
            .map(|r| r.ecosystem.display_name())
            .collect();
        assert_eq!(order, ["Cargo", "npm", "NuGet", "pip"]);
    }

    #[test]
    fn dep_tier_boundaries() {
        assert_eq!(DepTier::from_drift(0.0), DepTier::Fresh);
        assert_eq!(DepTier::from_drift(0.49), DepTier::Fresh);
        assert_eq!(DepTier::from_drift(0.5), DepTier::Aging);
        assert_eq!(DepTier::from_drift(1.99), DepTier::Aging);
        assert_eq!(DepTier::from_drift(2.0), DepTier::Stale);
        assert_eq!(DepTier::from_drift(4.99), DepTier::Stale);
        assert_eq!(DepTier::from_drift(5.0), DepTier::Critical);
        assert_eq!(DepTier::from_drift(10.0), DepTier::Critical);
    }

    #[test]
    fn critical_callout_by_drift() {
        let dep = DepAge {
            name: "old-crate".into(),
            ecosystem: Ecosystem::Cargo,
            current_version: "0.1.0".into(),
            drift_years: 6.0,
            tier: DepTier::Critical,
            vulnerabilities: vec![],
        };
        assert!(dep.is_critical_callout());
    }

    #[test]
    fn critical_callout_by_vuln() {
        let dep = DepAge {
            name: "vulnerable-pkg".into(),
            ecosystem: Ecosystem::Npm,
            current_version: "1.0.0".into(),
            drift_years: 0.3,
            tier: DepTier::Fresh,
            vulnerabilities: vec![Vuln {
                id: "CVE-2024-1234".into(),
                severity: "HIGH".into(),
                description: "RCE vulnerability".into(),
            }],
        };
        assert!(dep.is_critical_callout());
    }

    #[test]
    fn ecosystem_osv_names() {
        assert_eq!(Ecosystem::Cargo.osv_name(), "crates.io");
        assert_eq!(Ecosystem::Npm.osv_name(), "npm");
        assert_eq!(Ecosystem::Pip.osv_name(), "PyPI");
        assert_eq!(Ecosystem::Nuget.osv_name(), "NuGet");
    }
}
