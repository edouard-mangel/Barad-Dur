//! Author-centric views: per-file ownership shares and the contributor
//! cards (commit cadence, message hygiene, focus areas).

use std::collections::HashMap;

use crate::snapshot::RepoSnapshot;

use super::super::actions::score_commit_message;
use super::super::types::{AuthorCard, AuthorShare, FileOwnership};

pub(crate) fn build_author_ownership(snapshot: &RepoSnapshot) -> Vec<FileOwnership> {
    snapshot
        .blame_map
        .iter()
        .map(|(path, lines)| {
            let mut author_counts: HashMap<usize, usize> = HashMap::new();
            for line in lines {
                *author_counts.entry(line.author_id).or_insert(0) += line.line_count;
            }
            let total: usize = lines.iter().map(|l| l.line_count).sum::<usize>().max(1);
            let mut authors: Vec<AuthorShare> = author_counts
                .into_iter()
                .map(|(id, count)| {
                    let name = snapshot
                        .authors
                        .get(id)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| format!("author-{}", id));
                    AuthorShare {
                        name,
                        pct: count as f64 / total as f64 * 100.0,
                    }
                })
                .collect();
            authors.sort_by(|a, b| b.pct.partial_cmp(&a.pct).unwrap());
            FileOwnership {
                path: path.to_string_lossy().to_string(),
                authors,
            }
        })
        .collect()
}

pub(crate) fn build_author_cards(snapshot: &RepoSnapshot) -> Vec<AuthorCard> {
    let now = chrono::Utc::now();

    // Pre-compute per-author blame lines across all files
    let mut author_lines: HashMap<usize, usize> = HashMap::new();
    let mut author_file_pcts: HashMap<usize, Vec<(String, f64)>> = HashMap::new();
    let mut author_files_owned: HashMap<usize, usize> = HashMap::new();

    for (path, blame_lines) in &snapshot.blame_map {
        let total: usize = blame_lines
            .iter()
            .map(|b| b.line_count)
            .sum::<usize>()
            .max(1);
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for bl in blame_lines {
            *counts.entry(bl.author_id).or_insert(0) += bl.line_count;
        }
        for (&author_id, &count) in &counts {
            *author_lines.entry(author_id).or_insert(0) += count;
            let pct = count as f64 / total as f64 * 100.0;
            author_file_pcts
                .entry(author_id)
                .or_default()
                .push((path.to_string_lossy().to_string(), pct));
            if pct > 50.0 {
                *author_files_owned.entry(author_id).or_insert(0) += 1;
            }
        }
    }

    let mut cards: Vec<AuthorCard> = snapshot
        .authors
        .iter()
        .map(|author| {
            let commit_ids = snapshot
                .commits_by_author
                .get(&author.id)
                .cloned()
                .unwrap_or_default();

            let author_commits: Vec<&crate::snapshot::Commit> = commit_ids
                .iter()
                .filter_map(|cid| snapshot.commits.iter().find(|c| c.id == *cid))
                .collect();

            let commit_count = author_commits.len();

            let last_active = author_commits
                .iter()
                .map(|c| c.timestamp)
                .max()
                .unwrap_or(snapshot.created_at);
            let days_since_active = (now - last_active).num_days().max(0);

            let avg_commit_quality = if author_commits.is_empty() {
                0.0
            } else {
                let total_q: f64 = author_commits
                    .iter()
                    .map(|c| score_commit_message(&c.message))
                    .sum();
                total_q / author_commits.len() as f64
            };

            let mut file_pcts = author_file_pcts
                .get(&author.id)
                .cloned()
                .unwrap_or_default();
            file_pcts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_files: Vec<String> = file_pcts.iter().take(5).map(|(p, _)| p.clone()).collect();

            let mut dirs = std::collections::HashSet::new();
            for commit in &author_commits {
                for fc in &commit.files_changed {
                    if let Some(parent) = fc.path.parent() {
                        dirs.insert(parent.to_string_lossy().to_string());
                    }
                }
            }

            AuthorCard {
                name: author.name.clone(),
                email: author.email.clone(),
                commit_count,
                files_owned: *author_files_owned.get(&author.id).unwrap_or(&0),
                lines_owned: *author_lines.get(&author.id).unwrap_or(&0),
                avg_commit_quality,
                top_files,
                last_active,
                days_since_active,
                directories_touched: dirs.len(),
            }
        })
        .collect();

    cards.sort_by_key(|c| std::cmp::Reverse(c.commit_count));
    cards
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Author, BlameLine, TimeWindow};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_test_snapshot_with_blame(
        authors: Vec<(&str, &str)>,
        blame_entries: Vec<(&str, Vec<BlameLine>)>,
    ) -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.authors = authors
            .into_iter()
            .enumerate()
            .map(|(i, (name, email))| Author {
                id: i,
                name: name.to_string(),
                email: email.to_string(),
            })
            .collect();
        snapshot.blame_map = blame_entries
            .into_iter()
            .map(|(path, lines)| (PathBuf::from(path), lines))
            .collect();
        snapshot
    }

    fn blame(author_id: usize, line_count: usize) -> BlameLine {
        BlameLine {
            author_id,
            timestamp: Utc::now(),
            line_count,
        }
    }

    #[test]
    fn ownership_single_author_uncompressed() {
        let snapshot = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com")],
            vec![("main.rs", vec![blame(0, 1), blame(0, 1), blame(0, 1)])],
        );

        let ownership = build_author_ownership(&snapshot);
        assert_eq!(ownership.len(), 1);
        assert_eq!(ownership[0].authors.len(), 1);
        assert!((ownership[0].authors[0].pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ownership_single_author_rle_compressed() {
        let snapshot = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com")],
            vec![("main.rs", vec![blame(0, 50)])],
        );

        let ownership = build_author_ownership(&snapshot);
        assert_eq!(ownership[0].authors[0].pct, 100.0);
    }

    #[test]
    fn ownership_two_authors_uncompressed() {
        let snapshot = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com"), ("Bob", "bob@x.com")],
            vec![(
                "main.rs",
                vec![blame(0, 1), blame(0, 1), blame(0, 1), blame(1, 1)],
            )],
        );

        let ownership = build_author_ownership(&snapshot);
        let file = &ownership[0];
        // Alice: 3/4 = 75%, Bob: 1/4 = 25%
        assert_eq!(file.authors[0].name, "Alice");
        assert!((file.authors[0].pct - 75.0).abs() < f64::EPSILON);
        assert_eq!(file.authors[1].name, "Bob");
        assert!((file.authors[1].pct - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ownership_two_authors_rle_gives_same_result_as_uncompressed() {
        // RLE: Alice owns 30 lines, Bob owns 10 lines → 75% / 25%
        let snapshot_rle = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com"), ("Bob", "bob@x.com")],
            vec![("main.rs", vec![blame(0, 30), blame(1, 10)])],
        );
        // Uncompressed equivalent: same 40 lines, one entry per line
        let mut uncompressed_lines = vec![blame(0, 1); 30];
        uncompressed_lines.extend(vec![blame(1, 1); 10]);
        let snapshot_flat = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com"), ("Bob", "bob@x.com")],
            vec![("main.rs", uncompressed_lines)],
        );

        let own_rle = build_author_ownership(&snapshot_rle);
        let own_flat = build_author_ownership(&snapshot_flat);

        for (r, f) in own_rle[0].authors.iter().zip(own_flat[0].authors.iter()) {
            assert_eq!(r.name, f.name);
            assert!((r.pct - f.pct).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn ownership_empty_blame_map_returns_empty() {
        let snapshot = make_test_snapshot_with_blame(vec![("Alice", "alice@x.com")], vec![]);
        let ownership = build_author_ownership(&snapshot);
        assert!(ownership.is_empty());
    }
}
