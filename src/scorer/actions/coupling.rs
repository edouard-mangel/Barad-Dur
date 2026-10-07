use std::collections::HashMap;
use std::path::Path;

use crate::scorer::types::{ActionItem, ReportTab};

pub(super) const CONTENT_ADVICE: &str =
    "Reaches into another module's internals — import through the module's public interface instead.";
pub(super) const COMMON_ADVICE: &str =
    "Shared mutable global state — replace it with explicitly passed or injected state.";
pub(super) const CONTROL_ADVICE: &str =
    "A flag parameter steers this function's control flow — split it into two intent-revealing functions.";
pub(super) const INHERITANCE_ADVICE: &str =
    "Deep inheritance chain — favor composition over inheritance, or flatten the hierarchy.";

/// Per-file coupling refactoring suggestions, ranked worst-rung-first
/// (Content≻Common≻Inheritance≻Control), corroborated-before-dormant within a rung, then
/// higher finding-count first, capped at 10. A file's action speaks to its
/// most severe rung. Empty when detection did not run.
pub(in crate::scorer) fn generate_coupling_actions(
    evidence: &crate::metrics::coupling::CouplingEvidence,
) -> Vec<ActionItem> {
    use crate::snapshot::CouplingKind;

    if !evidence.detection_ran {
        return Vec::new();
    }
    let findings = &evidence.findings;
    if findings.is_empty() {
        return Vec::new();
    }

    // severity index: lower = worse. Group by file, tracking worst rung + count.
    let mut by_file: HashMap<&Path, (u8, usize)> = HashMap::new();
    for f in findings {
        let sev = match f.kind {
            CouplingKind::Content => 0u8,
            CouplingKind::Common => 1,
            CouplingKind::Inheritance => 2,
            CouplingKind::Control => 3,
        };
        let entry = by_file.entry(f.path.as_path()).or_insert((sev, 0));
        entry.0 = entry.0.min(sev);
        entry.1 += 1;
    }

    let mut rows: Vec<(&Path, u8, bool, usize)> = by_file
        .into_iter()
        .map(|(path, (sev, count))| (path, sev, evidence.is_corroborated(path), count))
        .collect();
    // worst rung asc → corroborated first → count desc → path asc.
    rows.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(b.2.cmp(&a.2))
            .then(b.3.cmp(&a.3))
            .then(a.0.cmp(b.0))
    });

    rows.into_iter()
        .take(10)
        .map(|(path, sev, corroborated, count)| {
            let (kind_label, advice) = match sev {
                0 => ("content", CONTENT_ADVICE),
                1 => ("common", COMMON_ADVICE),
                2 => ("inheritance", INHERITANCE_ADVICE),
                _ => ("control", CONTROL_ADVICE),
            };
            let corr_note = if corroborated {
                ", corroborated by change history"
            } else {
                ""
            };
            ActionItem {
                text: format!(
                    "[Coupling] {} — {} finding(s) (worst: {}){} — {}",
                    path.display(),
                    count,
                    kind_label,
                    corr_note,
                    advice
                ),
                target_tab: Some(ReportTab::Coupling),
                sort_by: None,
            }
        })
        .collect()
}
