use crate::metrics::file_role::{classify, FileRole};
use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

/// Files in top-quartile churn that are also dominated by a single author.
///
/// High churn indicates frequent change; single-author ownership means only one
/// person understands those changes. The combination is a knowledge-risk hotspot:
/// the file is both hard to maintain and hard to hand off.
pub(super) fn churn_ownership_risk(snapshot: &RepoSnapshot) -> MetricValue {
    // Skip on solo projects (ownership concentration is expected there).
    if snapshot.authors.len() <= 1 {
        return MetricValue {
            name: "Churn-ownership risk".to_string(),
            description: "Solo project — not applicable".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    if snapshot.blame_map.is_empty() || snapshot.commits_by_file.is_empty() {
        return MetricValue {
            name: "Churn-ownership risk".to_string(),
            description: "No blame or churn data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let churn_p75 = percentile_75(snapshot.commits_by_file.values().map(|c| c.len()).collect());

    let risky: Vec<String> = snapshot
        .blame_map
        .iter()
        .filter(|(path, lines)| {
            if classify(path) != FileRole::Source {
                return false;
            }
            // Must be above the churn threshold.
            let churn = snapshot
                .commits_by_file
                .get(*path)
                .map(|c| c.len())
                .unwrap_or(0);
            if churn <= churn_p75 {
                return false;
            }
            // Must be strongly dominated by a single author (>80% of blame lines).
            is_single_author_dominated(lines)
        })
        .map(|(p, _)| p.display().to_string())
        .collect();

    let count = risky.len();
    let source_total = snapshot
        .blame_map
        .keys()
        .filter(|path| classify(path) == FileRole::Source)
        .count();
    let pct = if source_total == 0 {
        0.0
    } else {
        count as f64 / source_total as f64 * 100.0
    };
    MetricValue {
        name: "Churn-ownership risk".to_string(),
        description: format!(
            "{count}/{source_total} source files combine high churn with >80% ownership by one author ({pct:.1}%) — advisory; clear ownership alone is not continuity risk"
        ),
        raw_value: RawValue::List(risky),
        score: None,
    }
}

fn percentile_75(mut values: Vec<usize>) -> usize {
    values.sort_unstable();
    values
        .get(values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or(0)
}

fn is_single_author_dominated(lines: &[crate::snapshot::BlameLine]) -> bool {
    let counts = crate::metrics::author_line_counts(lines);
    let total: usize = counts.values().sum();
    counts
        .into_iter()
        .filter(|(author, _)| *author != crate::metrics::UNKNOWN_AUTHOR)
        .any(|(_, count)| total > 0 && count as f64 / total as f64 > 0.8)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::metrics::testutil::make_snapshot;
    use crate::snapshot::*;
    use chrono::Utc;

    fn add_authors(s: &mut RepoSnapshot, names: &[&str]) {
        for (i, name) in names.iter().enumerate() {
            s.authors.push(Author {
                id: i,
                name: name.to_string(),
                email: format!("{}@test.com", name.to_lowercase()),
            });
        }
    }

    fn blame_dominated(author_id: usize, line_count: usize) -> Vec<BlameLine> {
        // Single author dominates with `line_count` lines.
        let now = Utc::now();
        vec![
            BlameLine {
                author_id,
                timestamp: now,
                line_count,
            },
            BlameLine {
                author_id: author_id + 1,
                timestamp: now,
                line_count: line_count / 10, // minority author
            },
        ]
    }

    fn blame_shared(line_count: usize) -> Vec<BlameLine> {
        // 50/50 split — neither author dominates.
        let now = Utc::now();
        vec![
            BlameLine {
                author_id: 0,
                timestamp: now,
                line_count,
            },
            BlameLine {
                author_id: 1,
                timestamp: now,
                line_count,
            },
        ]
    }

    #[test]
    fn solo_project_returns_na() {
        let mut s = make_snapshot();
        add_authors(&mut s, &["Alice"]);
        s.blame_map
            .insert(PathBuf::from("a.rs"), blame_dominated(0, 100));
        s.commits_by_file
            .insert(PathBuf::from("a.rs"), (0..30u32).map(CommitId).collect());
        let m = churn_ownership_risk(&s);
        assert_eq!(m.score, None);
        assert!(matches!(m.raw_value, RawValue::Text(_)));
    }

    #[test]
    fn no_blame_data_has_no_score() {
        let mut s = make_snapshot();
        add_authors(&mut s, &["Alice", "Bob"]);
        let m = churn_ownership_risk(&s);
        assert_eq!(m.score, None);
    }

    #[test]
    fn no_churn_data_has_no_score() {
        // blame_map is non-empty but commits_by_file is empty
        let mut s = make_snapshot();
        add_authors(&mut s, &["Alice", "Bob"]);
        s.blame_map
            .insert(PathBuf::from("a.rs"), blame_dominated(0, 100));
        // commits_by_file intentionally left empty
        let m = churn_ownership_risk(&s);
        assert_eq!(m.score, None);
    }

    #[test]
    fn high_churn_single_owner_is_flagged() {
        let mut s = make_snapshot();
        add_authors(&mut s, &["Alice", "Bob"]);
        // "risky.rs": Alice dominates, churn=10 (above p75 of [10,5,3,2])
        let files: &[(&str, usize, usize)] = &[
            ("risky.rs", 90, 10),
            ("ok1.rs", 50, 5),
            ("ok2.rs", 50, 3),
            ("ok3.rs", 50, 2),
        ];
        for (name, owner_lines, churn) in files {
            s.blame_map
                .insert(PathBuf::from(name), blame_dominated(0, *owner_lines));
            s.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn as u32).map(CommitId).collect(),
            );
        }
        let m = churn_ownership_risk(&s);
        assert_eq!(m.score, None, "ownership concentration is advisory");
        match &m.raw_value {
            RawValue::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn high_churn_shared_ownership_not_flagged() {
        let mut s = make_snapshot();
        add_authors(&mut s, &["Alice", "Bob"]);
        // "shared.rs": 50/50 ownership, high churn — should NOT be flagged
        s.blame_map
            .insert(PathBuf::from("shared.rs"), blame_shared(50));
        s.commits_by_file.insert(
            PathBuf::from("shared.rs"),
            (0..20u32).map(CommitId).collect(),
        );
        for i in 1..4usize {
            s.blame_map
                .insert(PathBuf::from(format!("ok{}.rs", i)), blame_shared(50));
            s.commits_by_file.insert(
                PathBuf::from(format!("ok{}.rs", i)),
                (0..i as u32).map(CommitId).collect(),
            );
        }
        let m = churn_ownership_risk(&s);
        assert_eq!(m.score, None, "ownership concentration is advisory");
    }

    #[test]
    fn low_churn_single_owner_not_flagged() {
        let mut s = make_snapshot();
        add_authors(&mut s, &["Alice", "Bob"]);
        // Single owner but churn = 1 per file — all at or below p75
        for i in 0..4usize {
            s.blame_map
                .insert(PathBuf::from(format!("f{}.rs", i)), blame_dominated(0, 100));
            s.commits_by_file.insert(
                PathBuf::from(format!("f{}.rs", i)),
                vec![CommitId(i as u32)],
            );
        }
        let m = churn_ownership_risk(&s);
        assert_eq!(m.score, None, "ownership concentration is advisory");
    }
}
