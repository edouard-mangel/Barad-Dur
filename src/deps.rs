use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
pub struct Vuln {
    pub id: String,
    pub severity: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct EcosystemReport {
    pub ecosystem: Ecosystem,
    pub total_deps: usize,
    pub mean_drift_years: f64,
    pub total_drift_years: f64,
    pub critical_deps: Vec<DepAge>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
