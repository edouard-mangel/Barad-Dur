//! Repo-level churn timeline (Crime Scene Ch. 14, trends design M1):
//! day-bucketed lines added/deleted across the analysis window. Pure
//! `(snapshot) → value`; merge commits excluded (their first-parent diff
//! double-counts every merged MR); zero-filled between the first and last
//! active day so spike/silence shapes survive serialization.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use chrono::NaiveDate;

use crate::scorer::{ChurnBucket, ChurnTimelineReport};
use crate::snapshot::RepoSnapshot;

/// Build the report's `churn_timeline` section. `None` when the window
/// holds no non-merge commits — no shape to report.
pub(crate) fn churn_timeline_report(snapshot: &RepoSnapshot) -> Option<ChurnTimelineReport> {
    let known: HashSet<&PathBuf> = snapshot.files.iter().map(|f| &f.path).collect();
    // Per UTC day: (added, deleted) over known files, non-merge commits.
    let per_day: BTreeMap<NaiveDate, (u64, u64)> = snapshot
        .commits
        .iter()
        .filter(|c| !c.is_merge)
        .fold(BTreeMap::new(), |mut days, c| {
            let entry = days.entry(c.timestamp.date_naive()).or_insert((0, 0));
            for fc in c.files_changed.iter().filter(|fc| known.contains(&fc.path)) {
                entry.0 += u64::from(fc.additions);
                entry.1 += u64::from(fc.deletions);
            }
            days
        });
    let (&first, _) = per_day.first_key_value()?;
    let (&last, _) = per_day.last_key_value()?;
    let buckets = first
        .iter_days()
        .take_while(|d| *d <= last)
        .map(|day| {
            let (added, deleted) = per_day.get(&day).copied().unwrap_or((0, 0));
            ChurnBucket {
                date: day.format("%Y-%m-%d").to_string(),
                added,
                deleted,
            }
        })
        .collect();
    Some(ChurnTimelineReport {
        bucket_days: 1,
        merge_commits_excluded: true,
        buckets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::testutil::{make_file, make_snapshot};
    use crate::snapshot::{ChangeType, Commit, CommitId, FileChange};
    use chrono::{TimeZone, Utc};

    fn commit(id: u32, day: u32, hour: u32, files: &[(&str, u32, u32)], is_merge: bool) -> Commit {
        Commit {
            id: CommitId(id),
            author: 0,
            timestamp: Utc.with_ymd_and_hms(2026, 8, day, hour, 0, 0).unwrap(),
            message: String::new(),
            files_changed: files
                .iter()
                .map(|&(p, add, del)| FileChange {
                    path: p.into(),
                    additions: add,
                    deletions: del,
                    change_type: ChangeType::Modified,
                })
                .collect(),
            is_merge,
            parent_count: if is_merge { 2 } else { 1 },
        }
    }

    fn snap(commits: Vec<Commit>) -> crate::snapshot::RepoSnapshot {
        let mut s = make_snapshot();
        s.files = vec![make_file("a.rs"), make_file("b.rs")];
        s.commits = commits;
        s
    }

    #[test]
    fn buckets_by_utc_day_with_exact_sums() {
        // 23:59 and 00:01 land in adjacent buckets — both sides of the
        // day boundary.
        let s = snap(vec![
            commit(0, 19, 23, &[("a.rs", 10, 2)], false),
            commit(1, 20, 0, &[("a.rs", 5, 1), ("b.rs", 7, 0)], false),
        ]);
        let t = churn_timeline_report(&s).expect("report");
        assert_eq!(t.bucket_days, 1);
        assert!(t.merge_commits_excluded);
        assert_eq!(
            t.buckets,
            vec![
                ChurnBucket {
                    date: "2026-08-19".into(),
                    added: 10,
                    deleted: 2,
                },
                ChurnBucket {
                    date: "2026-08-20".into(),
                    added: 12,
                    deleted: 1,
                },
            ]
        );
    }

    #[test]
    fn gap_days_are_zero_filled() {
        let s = snap(vec![
            commit(0, 19, 9, &[("a.rs", 3, 0)], false),
            commit(1, 21, 9, &[("a.rs", 4, 0)], false),
        ]);
        let t = churn_timeline_report(&s).expect("report");
        assert_eq!(t.buckets.len(), 3, "19th, 20th (zero), 21st");
        assert_eq!(
            t.buckets[1],
            ChurnBucket {
                date: "2026-08-20".into(),
                added: 0,
                deleted: 0,
            }
        );
    }

    #[test]
    fn merge_commit_churn_is_excluded() {
        let s = snap(vec![
            commit(0, 19, 9, &[("a.rs", 3, 1)], false),
            commit(1, 19, 10, &[("a.rs", 500, 200), ("b.rs", 300, 0)], true),
        ]);
        let t = churn_timeline_report(&s).expect("report");
        assert_eq!(
            t.buckets,
            vec![ChurnBucket {
                date: "2026-08-19".into(),
                added: 3,
                deleted: 1,
            }]
        );
    }

    #[test]
    fn excluded_files_do_not_count() {
        // vendor.lock is not in snapshot.files (exclusion layers dropped
        // it) — its churn must not pollute the shape.
        let s = snap(vec![commit(
            0,
            19,
            9,
            &[("a.rs", 3, 0), ("vendor.lock", 9000, 8000)],
            false,
        )]);
        let t = churn_timeline_report(&s).expect("report");
        assert_eq!(
            t.buckets,
            vec![ChurnBucket {
                date: "2026-08-19".into(),
                added: 3,
                deleted: 0,
            }]
        );
    }

    #[test]
    fn no_non_merge_commits_yields_none() {
        assert!(churn_timeline_report(&snap(vec![])).is_none());
        let only_merge = snap(vec![commit(0, 19, 9, &[("a.rs", 1, 0)], true)]);
        assert!(churn_timeline_report(&only_merge).is_none());
    }
}
