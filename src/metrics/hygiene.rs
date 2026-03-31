use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_hygiene(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::HygieneThresholds,
) -> CategoryResult {
    let metrics = vec![
        commit_message_quality(snapshot, thresholds),
        history_cleanliness(snapshot, thresholds),
        gitignore_coverage(snapshot, thresholds),
    ];

    CategoryResult {
        name: "Git Hygiene".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

const CONVENTIONAL_PREFIXES: &[&str] = &[
    "feat:",
    "fix:",
    "docs:",
    "style:",
    "refactor:",
    "perf:",
    "test:",
    "chore:",
    "ci:",
    "build:",
    "revert:",
    "feat(",
    "fix(",
    "docs(",
    "style(",
    "refactor(",
    "perf(",
    "test(",
    "chore(",
    "ci(",
    "build(",
    "revert(",
];

/// Evaluate commit message quality.
fn commit_message_quality(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::HygieneThresholds,
) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "Commit message quality".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let window_commits: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| snapshot.time_window.contains(&c.timestamp))
        .collect();

    if window_commits.is_empty() {
        return MetricValue {
            name: "Commit message quality".to_string(),
            description: "No commits in window".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let total = window_commits.len();
    let good = window_commits
        .iter()
        .filter(|c| is_good_commit_message(&c.message))
        .count();
    let conventional = window_commits
        .iter()
        .filter(|c| is_conventional_commit(&c.message))
        .count();

    let quality_pct = (good as f64 / total as f64) * 100.0;
    let conventional_pct = (conventional as f64 / total as f64) * 100.0;

    let score = if quality_pct > 80.0 {
        90
    } else if quality_pct > 60.0 {
        70
    } else if quality_pct > 40.0 {
        50
    } else {
        30
    };

    MetricValue {
        name: "Commit message quality".to_string(),
        description: format!(
            "{:.0}% good messages, {:.0}% conventional commits",
            quality_pct, conventional_pct
        ),
        raw_value: RawValue::Percentage(quality_pct),
        score,
    }
}

fn is_good_commit_message(msg: &str) -> bool {
    let first_line = msg.lines().next().unwrap_or("");
    if first_line.len() < 10 {
        return false;
    }
    // Check for capitalization (after any conventional prefix)
    let subject = if let Some(pos) = first_line.find(": ") {
        &first_line[pos + 2..]
    } else {
        first_line
    };
    if subject.is_empty() {
        return false;
    }
    let first_char = subject.chars().next().unwrap();
    if !first_char.is_uppercase() && !is_conventional_commit(first_line) {
        return false;
    }
    // Not just "wip", "fix", "update" etc.
    let lower = first_line.to_lowercase();
    if lower == "wip" || lower == "fix" || lower == "update" || lower == "changes" {
        return false;
    }
    true
}

fn is_conventional_commit(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    CONVENTIONAL_PREFIXES.iter().any(|p| lower.starts_with(p))
}

/// History cleanliness based on merge hygiene.
fn history_cleanliness(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::HygieneThresholds,
) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "History cleanliness".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let total = snapshot.commits.len();
    let merge_count = snapshot.commits.iter().filter(|c| c.is_merge).count();
    let octopus_merges = snapshot
        .commits
        .iter()
        .filter(|c| c.parent_count > 2)
        .count();

    // Check for empty commit messages
    let empty_messages = snapshot
        .commits
        .iter()
        .filter(|c| c.message.trim().is_empty())
        .count();

    let merge_pct = if total > 0 {
        (merge_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let issues = octopus_merges + empty_messages;

    let score = if issues > 5 || merge_pct > 60.0 {
        30
    } else if issues > 2 || merge_pct > 40.0 {
        55
    } else if merge_pct > 20.0 {
        75
    } else {
        90
    };

    MetricValue {
        name: "History cleanliness".to_string(),
        description: format!(
            "{:.0}% merges, {} octopus merges, {} empty messages",
            merge_pct, octopus_merges, empty_messages
        ),
        raw_value: RawValue::Count(issues),
        score,
    }
}

const SUSPICIOUS_PATTERNS: &[&str] = &[
    ".env",
    ".env.",
    "credentials",
    "secret",
    ".key",
    ".pem",
    ".p12",
    ".pfx",
    "node_modules/",
    "__pycache__/",
    ".DS_Store",
    "Thumbs.db",
    ".pyc",
];

/// Check tracked files for suspicious patterns that should be in .gitignore.
fn gitignore_coverage(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::HygieneThresholds,
) -> MetricValue {
    let suspicious: Vec<String> = snapshot
        .files
        .iter()
        .filter(|f| {
            let path_str = f.path.to_string_lossy().to_lowercase();
            let file_name = f
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            SUSPICIOUS_PATTERNS
                .iter()
                .any(|pat| path_str.contains(pat) || file_name.ends_with(pat) || file_name == *pat)
        })
        .map(|f| f.path.display().to_string())
        .collect();

    let count = suspicious.len();

    let score = match count {
        0 => 100,
        1..=2 => 70,
        3..=5 => 45,
        _ => 20,
    };

    MetricValue {
        name: "Gitignore coverage".to_string(),
        description: if count > 0 {
            format!("{} suspicious tracked files", count)
        } else {
            "No suspicious tracked files".to_string()
        },
        raw_value: if suspicious.is_empty() {
            RawValue::Count(0)
        } else {
            RawValue::List(suspicious)
        },
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use chrono::{Duration, Utc};
    use std::path::PathBuf;

    #[test]
    fn commit_message_quality_scores() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        let messages = [
            "Add login feature with OAuth support",  // good
            "fix",                                   // bad (too short)
            "Update README with installation steps", // good
            "wip",                                   // bad
        ];

        for (i, msg) in messages.iter().enumerate() {
            snapshot.commits.push(Commit {
                id: CommitId(i as u32),
                author: 0,
                timestamp: now - Duration::days(i as i64 + 1),
                message: msg.to_string(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            });
        }

        let result =
            commit_message_quality(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 50.0).abs() < 1.0, "Expected ~50%, got {}", p),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn history_cleanliness_flags_issues() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        // 1 octopus merge + 1 empty message
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now,
                message: "msg".into(),
                files_changed: vec![],
                is_merge: true,
                parent_count: 3, // octopus
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now,
                message: "".into(), // empty
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(2),
                author: 0,
                timestamp: now,
                message: "Normal commit".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];

        let result = history_cleanliness(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 2, "1 octopus + 1 empty = 2 issues"),
            _ => panic!("Expected Count"),
        }
    }

    #[test]
    fn gitignore_detects_suspicious_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        snapshot.files = vec![
            FileEntry {
                path: ".env".into(),
                size_bytes: 50,
                is_binary: false,
                depth: 0,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "node_modules/package.json".into(),
                size_bytes: 100,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "app.log".into(),
                size_bytes: 1000,
                is_binary: false,
                depth: 0,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "src/main.rs".into(),
                size_bytes: 200,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
        ];

        let result = gitignore_coverage(&snapshot, &crate::config::HygieneThresholds::default());
        // .env and node_modules/ should be flagged
        match &result.raw_value {
            RawValue::List(items) => assert!(
                items.len() >= 2,
                "Expected at least 2 suspicious files, got {:?}",
                items
            ),
            _ => panic!("Expected List"),
        }
        assert!(result.score < 100);
    }
}
