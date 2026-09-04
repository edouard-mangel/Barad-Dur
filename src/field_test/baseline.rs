use crate::field_test::surface::DecisionSurface;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Where a repository's committed baseline lives.
pub fn baseline_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.surface.json"))
}

/// Write a baseline as pretty JSON with a trailing newline — these are
/// reviewed as git diffs, so readability is the point.
pub fn write_baseline(dir: &Path, name: &str, surface: &DecisionSurface) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating baseline dir {}", dir.display()))?;
    let path = baseline_path(dir, name);
    let body = format!("{}\n", serde_json::to_string_pretty(surface)?);
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))
}

/// Read a baseline. `Ok(None)` means this repository has no committed
/// baseline; callers decide whether that is an error or an explicit accept.
pub fn read_baseline(dir: &Path, name: &str) -> Result<Option<DecisionSurface>> {
    let path = baseline_path(dir, name);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let surface =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(surface))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::{
        ActionSurface, CategorySurface, DecisionSurface, MetricSurface,
    };
    use std::collections::BTreeMap;

    fn empty_surface() -> DecisionSurface {
        DecisionSurface {
            overall_score: Some(55),
            total_files: 1,
            total_commits: 2,
            total_authors: 3,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![],
            actions: vec![],
            top_hotspots: vec![],
        }
    }

    /// Non-empty on purpose: an empty surface can round-trip through a JSON
    /// encoder that silently discards fields. This fixture in particular
    /// carries an unscored metric (`score: None`) so the test can prove
    /// that `None` survives the JSON round trip rather than coming back as
    /// `Some(0)` — the exact regression this baseline store guards against.
    fn sample_surface() -> DecisionSurface {
        DecisionSurface {
            overall_score: Some(55),
            total_files: 1,
            total_commits: 2,
            total_authors: 3,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![CategorySurface {
                name: "health".to_string(),
                score: Some(80),
                metrics: vec![
                    MetricSurface {
                        name: "coverage".to_string(),
                        score: Some(90),
                    },
                    MetricSurface {
                        name: "flaky_tests".to_string(),
                        score: None,
                    },
                ],
            }],
            actions: vec![ActionSurface {
                target_tab: "health".to_string(),
                text: "Add tests for the flaky suite".to_string(),
            }],
            top_hotspots: vec!["src/lib.rs".to_string(), "src/main.rs".to_string()],
        }
    }

    #[test]
    fn round_trips_a_surface() {
        let dir = tempfile::tempdir().expect("tempdir");
        let s = sample_surface();
        write_baseline(dir.path(), "ripgrep", &s).expect("writes");
        let back = read_baseline(dir.path(), "ripgrep").expect("reads");
        assert_eq!(back, Some(s));
        // The equality above already covers this, but spell it out: an
        // unscored metric must come back as `None`, not `Some(0)`.
        let back = back.expect("baseline present");
        assert_eq!(back.categories[0].metrics[1].score, None);
    }

    #[test]
    fn missing_baseline_reads_as_none_for_the_caller_to_handle() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(read_baseline(dir.path(), "brand-new").expect("reads"), None);
    }

    #[test]
    fn malformed_baseline_is_an_error_not_a_silent_reset() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(baseline_path(dir.path(), "corrupt"), "not valid json")
            .expect("write garbage");
        assert!(
            read_baseline(dir.path(), "corrupt").is_err(),
            "a malformed baseline must surface as an error, never as Ok(None) — \
             collapsing it to None would let the next run silently overwrite \
             the corrupt file and destroy the committed record"
        );
    }

    #[test]
    fn writes_pretty_json_so_diffs_are_reviewable_in_git() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_baseline(dir.path(), "ripgrep", &empty_surface()).expect("writes");
        let raw = std::fs::read_to_string(baseline_path(dir.path(), "ripgrep")).expect("read");
        assert!(
            raw.matches('\n').count() > 1,
            "baseline must be pretty-printed (indented, one field per line), \
             not compact JSON with a single trailing newline appended"
        );
        assert!(raw.ends_with('\n'), "baseline must end with a newline");
    }
}
