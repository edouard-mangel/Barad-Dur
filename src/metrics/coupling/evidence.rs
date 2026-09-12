//! Coupling facts derived once per snapshot and configuration.
//!
//! The Coupling metric, the per-kind finding counts, hotspot badges,
//! coupling actions, and the gate ratchet all need the same answer to "which
//! findings count here?" — AST findings plus the config-gated barrel-bypass
//! and inheritance-depth findings, source files only. Deriving that answer in
//! one place, and handing the result to every consumer, is what keeps them
//! from disagreeing; it also stops each one re-walking the import graph and
//! class records for itself.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::CouplingThresholds;
use crate::snapshot::{CouplingFinding, CouplingKind, RepoSnapshot};

use super::{
    all_coupling_findings, corroboration_degree, detection_ran, has_detectable_files,
    CouplingFindingCounts,
};

#[derive(Debug, Clone)]
pub struct CouplingEvidence {
    /// Whether detection ran at all. An empty finding list without it means
    /// "not collected", never "clean".
    pub detection_ran: bool,
    /// Whether any file is in a language the detectors understand.
    pub has_detectable_files: bool,
    /// Every enabled finding, in the order the metrics list them: AST
    /// findings, then barrel-bypass (when the rule is on), then inheritance
    /// depth — source files only.
    pub findings: Vec<CouplingFinding>,
    /// path → distinct qualifying cross-boundary co-change partners; a
    /// finding whose path is present is "corroborated" (M5).
    pub corroboration: HashMap<PathBuf, usize>,
}

impl CouplingEvidence {
    pub fn derive(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> Self {
        Self {
            detection_ran: detection_ran(snapshot),
            has_detectable_files: has_detectable_files(snapshot),
            findings: all_coupling_findings(snapshot, thresholds),
            corroboration: corroboration_degree(snapshot, thresholds),
        }
    }

    /// Per-kind counts over the enabled findings. `None` when detection did
    /// not run or nothing was detectable — distinct from all-zero, which
    /// means "clean".
    pub fn finding_counts(&self) -> Option<CouplingFindingCounts> {
        (self.detection_ran && self.has_detectable_files).then(|| {
            let count =
                |kind: CouplingKind| self.findings.iter().filter(|f| f.kind == kind).count();
            CouplingFindingCounts {
                content: count(CouplingKind::Content),
                common: count(CouplingKind::Common),
                inheritance: count(CouplingKind::Inheritance),
                control: count(CouplingKind::Control),
            }
        })
    }

    pub fn is_corroborated(&self, path: &Path) -> bool {
        self.corroboration.contains_key(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::testutil::{make_file, make_snapshot};

    /// Cross-component import that bypasses src/a's barrel.
    fn snapshot_with_barrel_bypass() -> RepoSnapshot {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![
            make_file("src/a/index.ts"),
            make_file("src/a/impl.ts"),
            make_file("src/b/user.ts"),
        ];
        snapshot
            .import_graph
            .insert("src/b/user.ts".into(), vec!["src/a/impl.ts".into()]);
        snapshot
    }

    #[test]
    fn barrel_findings_join_the_enabled_set_only_when_the_rule_is_on() {
        let on = CouplingThresholds::default();
        assert!(on.content_barrel_rule, "default toggle must be on");
        let snapshot = snapshot_with_barrel_bypass();
        let evidence = CouplingEvidence::derive(&snapshot, &on);
        assert_eq!(evidence.findings.len(), 1);
        assert!(evidence.findings[0].evidence.contains("barrel"));

        let off = CouplingThresholds {
            content_barrel_rule: false,
            ..CouplingThresholds::default()
        };
        assert!(
            CouplingEvidence::derive(&snapshot, &off)
                .findings
                .is_empty(),
            "toggle off: barrel findings are not enabled anywhere"
        );
    }

    #[test]
    fn counts_are_absent_without_detection_even_when_findings_exist() {
        // Barrel bypass is graph-derived, so it is listed even though no
        // file metrics exist — but counts must not claim detection ran.
        let evidence = CouplingEvidence::derive(
            &snapshot_with_barrel_bypass(),
            &CouplingThresholds::default(),
        );
        assert!(!evidence.detection_ran);
        assert_eq!(evidence.findings.len(), 1);
        assert_eq!(evidence.finding_counts(), None);
    }

    #[test]
    fn counts_are_absent_when_no_file_is_in_a_detectable_language() {
        let mut snapshot = make_snapshot();
        snapshot.files = vec![make_file("notes.txt")];
        snapshot
            .file_metrics
            .insert("notes.txt".into(), Default::default());
        let evidence = CouplingEvidence::derive(&snapshot, &CouplingThresholds::default());
        assert!(evidence.detection_ran);
        assert!(!evidence.has_detectable_files);
        assert_eq!(evidence.finding_counts(), None);
    }
}
