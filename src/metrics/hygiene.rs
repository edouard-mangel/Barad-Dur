use crate::metrics::file_role::{classify, has_source_extension, FileRole};
use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;
use std::path::Path;

pub fn compute_hygiene(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::HygieneThresholds,
) -> CategoryResult {
    let metrics = vec![
        commit_message_quality(snapshot, thresholds),
        history_cleanliness(snapshot, thresholds),
        gitignore_coverage(snapshot, thresholds),
        firefighting_ratio(snapshot, thresholds),
        friction_language_ratio(snapshot, thresholds),
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
            score: None,
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
            score: None,
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
        score: Some(score),
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
            score: None,
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
        score: Some(score),
    }
}

const SUSPICIOUS_DIRECTORY_NAMES: &[&str] = &["node_modules", "__pycache__"];
const SUSPICIOUS_EXACT_FILE_NAMES: &[&str] = &[".ds_store", "thumbs.db"];
const SUSPICIOUS_EXTENSIONS: &[&str] = &["key", "pem", "p12", "pfx", "pyc"];
const SAFE_ENV_TEMPLATE_NAMES: &[&str] = &[".env.example", ".env.sample", ".env.template"];
const SENSITIVE_NAME_TOKENS: &[&str] = &["secret", "secrets", "credential", "credentials"];

fn has_suspicious_directory(path: &Path) -> bool {
    path.parent().is_some_and(|parent| {
        parent.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|name| {
                SUSPICIOUS_DIRECTORY_NAMES
                    .iter()
                    .any(|candidate| name.eq_ignore_ascii_case(candidate))
            })
        })
    })
}

fn is_env_file(name: &str) -> bool {
    if SAFE_ENV_TEMPLATE_NAMES
        .iter()
        .any(|template| name.eq_ignore_ascii_case(template))
    {
        return false;
    }

    name.eq_ignore_ascii_case(".env")
        || name
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(".env."))
}

fn has_sensitive_name_token(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();

    stem.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| {
            SENSITIVE_NAME_TOKENS
                .iter()
                .any(|candidate| token.eq_ignore_ascii_case(candidate))
        })
}

fn suspicious_tracked_reason(path: &Path) -> Option<&'static str> {
    if has_suspicious_directory(path) {
        return Some("generated or dependency directory");
    }

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if is_env_file(name) {
        return Some("local environment file");
    }
    if SUSPICIOUS_EXACT_FILE_NAMES
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
    {
        return Some("generated OS metadata");
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    if SUSPICIOUS_EXTENSIONS
        .iter()
        .any(|candidate| extension.eq_ignore_ascii_case(candidate))
    {
        return Some("sensitive or generated extension");
    }

    if has_sensitive_name_token(path) {
        if has_source_extension(path) || classify(path) == FileRole::Docs {
            return None;
        }
        return Some("sensitive filename token");
    }

    None
}

/// Check tracked files for suspicious patterns that should be in .gitignore.
fn gitignore_coverage(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::HygieneThresholds,
) -> MetricValue {
    let suspicious: Vec<String> = snapshot
        .files
        .iter()
        .filter(|file| suspicious_tracked_reason(&file.path).is_some())
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
        score: Some(score),
    }
}

/// Common English inflectional suffixes tolerated after a keyword match —
/// "hotfixed" (hotfix+ed), "reverting" (revert+ing), "workarounds"
/// (workaround+s) are genuine occurrences of the keyword. Deliberately
/// small and generic (not a real stemmer): a suffix outside this set, like
/// "athon" in "hackathon", means the word is unrelated to the keyword, not
/// an inflection of it.
const INFLECTION_SUFFIXES: &[&str] = &["s", "es", "d", "ed", "ing"];

/// True if `message` contains any of `keywords` as a whole word, or that
/// word plus a common inflectional suffix — not as a substring of an
/// unrelated word (e.g. "hack" must not match "hackathon", "hotfix" must
/// not match "HotfixManager", but "hotfix" must match "hotfixed"). Shared
/// by `firefighting_ratio` and `friction_language_ratio`, whose keyword
/// lists are otherwise disjoint.
fn message_contains_keyword(message: &str, keywords: &[&str]) -> bool {
    let lower = message.to_lowercase();
    let words = lower.split(|c: char| !c.is_ascii_alphanumeric());
    words.filter(|w| !w.is_empty()).any(|w| {
        keywords.iter().any(|kw| {
            w == *kw || (w.starts_with(kw) && INFLECTION_SUFFIXES.contains(&&w[kw.len()..]))
        })
    })
}

/// Shared shape for a "percentage of commits whose message contains one of
/// `keywords`" hygiene metric: window filtering, the N/A-on-empty branch,
/// and the score ladder are identical between `firefighting_ratio` and
/// `friction_language_ratio` — only the metric name, keyword list, and
/// description wording differ, so those are the only parameters.
fn keyword_commit_ratio(
    snapshot: &RepoSnapshot,
    metric_name: &str,
    keywords: &[&str],
    describe: impl Fn(usize, f64, usize) -> String,
) -> MetricValue {
    let window_commits: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| !c.is_merge && snapshot.time_window.contains(&c.timestamp))
        .collect();

    if window_commits.is_empty() {
        return MetricValue {
            name: metric_name.to_string(),
            description: "No commits in window".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let matched = window_commits
        .iter()
        .filter(|c| message_contains_keyword(&c.message, keywords))
        .count();

    let total = window_commits.len();
    let pct = (matched as f64 / total as f64) * 100.0;

    let score = if pct < 2.0 {
        90
    } else if pct < 5.0 {
        75
    } else if pct < 10.0 {
        55
    } else if pct < 20.0 {
        35
    } else {
        20
    };

    MetricValue {
        name: metric_name.to_string(),
        description: describe(matched, pct, total),
        raw_value: RawValue::Percentage(pct),
        score: Some(score),
    }
}

const FIREFIGHTING_KEYWORDS: &[&str] = &["revert", "hotfix", "emergency", "rollback"];

/// Percentage of commits that are reactive firefighting work (reverts, hotfixes, rollbacks).
/// High ratios signal unreliable tests, missing staging, or deploy process issues.
fn firefighting_ratio(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::HygieneThresholds,
) -> MetricValue {
    keyword_commit_ratio(
        snapshot,
        "Firefighting ratio",
        FIREFIGHTING_KEYWORDS,
        |matched, pct, total| {
            format!("{matched} firefighting commits ({pct:.1}% of {total} non-merge commits)")
        },
    )
}

const FRICTION_KEYWORDS: &[&str] = &[
    "hack",
    "workaround",
    "kludge",
    "temporary",
    "fixme",
    "sorry",
];

/// Percentage of commits whose message admits technical-debt friction
/// (hacks, workarounds, temporary fixes) — a different social signal than
/// `firefighting_ratio`'s reactive-incident-response keywords: this one
/// signals debt knowingly shipped, not something that broke.
fn friction_language_ratio(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::HygieneThresholds,
) -> MetricValue {
    keyword_commit_ratio(
        snapshot,
        "Friction language ratio",
        FRICTION_KEYWORDS,
        |matched, pct, total| {
            format!(
                "{matched} commit(s) admitting technical-debt friction ({pct:.1}% of {total} non-merge commits)"
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use chrono::{Duration, Utc};
    use std::path::PathBuf;

    fn snapshot_with_files(paths: &[&str]) -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = paths
            .iter()
            .map(|path| FileEntry {
                path: (*path).into(),
                size_bytes: 1,
                is_binary: false,
                depth: 0,
                blob_oid: String::new(),
            })
            .collect();
        snapshot
    }

    fn gitignore_findings(paths: &[&str]) -> MetricValue {
        gitignore_coverage(
            &snapshot_with_files(paths),
            &crate::config::HygieneThresholds::default(),
        )
    }

    #[test]
    fn firefighting_ratio_detects_reactive_commits() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        let messages = [
            "feat: add login page",       // normal
            "revert: undo bad deploy",    // firefighting
            "fix: typo in README",        // normal
            "hotfix: prod is down",       // firefighting
            "refactor: clean up modules", // normal
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

        let result = firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        // 2 out of 5 non-merge commits = 40%
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 40.0).abs() < 1.0, "Expected 40%, got {}", p),
            _ => panic!("Expected Percentage"),
        }
        assert!(
            result.score.unwrap() <= 35,
            "40% firefighting should score ≤35, got {:?}",
            result.score
        );
    }

    #[test]
    fn firefighting_ratio_ignores_merge_commits() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        // Merge commits should not count toward total
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "Merge branch main".into(),
                files_changed: vec![],
                is_merge: true,
                parent_count: 2,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "revert bad change".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(2),
                author: 0,
                timestamp: now - Duration::days(3),
                message: "feat: new feature".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];

        let result = firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        // 1 firefighting out of 2 non-merge = 50%
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 50.0).abs() < 1.0, "Expected 50%, got {}", p),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn firefighting_ratio_all_keywords_detected() {
        let now = Utc::now();
        for (msg, label) in &[
            ("revert: undo bad deploy", "revert"),
            ("hotfix: prod outage", "hotfix"),
            ("emergency: patch xss", "emergency"),
            ("rollback: bad migration", "rollback"),
        ] {
            let mut snapshot = RepoSnapshot::new(
                PathBuf::from("/tmp"),
                "test".into(),
                "main".into(),
                TimeWindow::default(),
            );
            snapshot.commits = vec![
                Commit {
                    id: CommitId(0),
                    author: 0,
                    timestamp: now - Duration::days(1),
                    message: msg.to_string(),
                    files_changed: vec![],
                    is_merge: false,
                    parent_count: 1,
                },
                Commit {
                    id: CommitId(1),
                    author: 0,
                    timestamp: now - Duration::days(2),
                    message: "feat: normal commit".into(),
                    files_changed: vec![],
                    is_merge: false,
                    parent_count: 1,
                },
            ];
            let result =
                firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
            match result.raw_value {
                RawValue::Percentage(p) => assert!(
                    (p - 50.0).abs() < 1.0,
                    "keyword '{}' should yield 50%, got {}",
                    label,
                    p
                ),
                _ => panic!("Expected Percentage for keyword '{}'", label),
            }
        }
    }

    #[test]
    fn firefighting_ratio_zero_percent_scores_highest() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "feat: add login".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "refactor: extract module".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        let result = firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        assert_eq!(result.score, Some(90), "0% firefighting should score 90");
    }

    #[test]
    fn firefighting_ratio_returns_na_when_no_commits_in_window() {
        let snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // No commits added — window_commits will be empty
        let result = firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Text(ref s) => assert_eq!(s, "N/A"),
            _ => panic!("Expected Text(N/A) for empty commit list"),
        }
        assert_eq!(result.score, None);
    }

    #[test]
    fn firefighting_ratio_is_case_insensitive() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![Commit {
            id: CommitId(0),
            author: 0,
            timestamp: now - Duration::days(1),
            message: "HOTFIX: PROD IS ON FIRE".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        }];
        let result = firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 100.0).abs() < 1.0),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn friction_language_ratio_detects_friction_commits() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        let messages = [
            "feat: add login page",         // normal
            "hack: quick fix for the demo", // friction
            "fix: typo in README",          // normal
            "workaround for flaky CI",      // friction
            "refactor: clean up modules",   // normal
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
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 40.0).abs() < 1.0, "Expected 40%, got {}", p),
            _ => panic!("Expected Percentage"),
        }
        assert!(
            result.score.unwrap() <= 35,
            "40% friction language should score ≤35, got {:?}",
            result.score
        );
    }

    #[test]
    fn friction_language_ratio_ignores_merge_commits() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "Merge branch main".into(),
                files_changed: vec![],
                is_merge: true,
                parent_count: 2,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "hack: temporary fix".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(2),
                author: 0,
                timestamp: now - Duration::days(3),
                message: "feat: new feature".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];

        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 50.0).abs() < 1.0, "Expected 50%, got {}", p),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn friction_language_ratio_all_keywords_detected() {
        let now = Utc::now();
        for (msg, label) in &[
            ("hack: quick patch", "hack"),
            ("workaround for the bug", "workaround"),
            ("kludge to unblock release", "kludge"),
            ("temporary disable of the check", "temporary"),
            ("fixme: revisit this later", "fixme"),
            ("sorry, this is ugly", "sorry"),
        ] {
            let mut snapshot = RepoSnapshot::new(
                PathBuf::from("/tmp"),
                "test".into(),
                "main".into(),
                TimeWindow::default(),
            );
            snapshot.commits = vec![
                Commit {
                    id: CommitId(0),
                    author: 0,
                    timestamp: now - Duration::days(1),
                    message: msg.to_string(),
                    files_changed: vec![],
                    is_merge: false,
                    parent_count: 1,
                },
                Commit {
                    id: CommitId(1),
                    author: 0,
                    timestamp: now - Duration::days(2),
                    message: "feat: normal commit".into(),
                    files_changed: vec![],
                    is_merge: false,
                    parent_count: 1,
                },
            ];
            let result =
                friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
            match result.raw_value {
                RawValue::Percentage(p) => assert!(
                    (p - 50.0).abs() < 1.0,
                    "keyword '{}' should yield 50%, got {}",
                    label,
                    p
                ),
                _ => panic!("Expected Percentage for keyword '{}'", label),
            }
        }
    }

    #[test]
    fn friction_language_ratio_zero_percent_scores_highest() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "feat: add login".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "refactor: extract module".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        assert_eq!(
            result.score,
            Some(90),
            "0% friction language should score 90"
        );
    }

    /// Builds `total` non-merge commits in-window, the first `friction_count`
    /// carrying a friction keyword and the rest a plain conventional message.
    fn friction_commits(total: usize, friction_count: usize) -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = (0..total)
            .map(|i| {
                let message = if i < friction_count {
                    format!("hack: friction commit {i}")
                } else {
                    format!("feat: normal commit {i}")
                };
                Commit {
                    id: CommitId(i as u32),
                    author: 0,
                    timestamp: now - Duration::days(i as i64 + 1),
                    message,
                    files_changed: vec![],
                    is_merge: false,
                    parent_count: 1,
                }
            })
            .collect();
        snapshot
    }

    #[test]
    fn friction_language_ratio_scores_75_just_below_5_percent_boundary() {
        // 1/30 = 3.33% — in [2.0, 5.0) → score 75.
        let snapshot = friction_commits(30, 1);
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 3.33).abs() < 0.1, "Expected ~3.33%, got {p}"),
            _ => panic!("Expected Percentage"),
        }
        assert_eq!(result.score, Some(75), "~3.3% friction should score 75");
    }

    #[test]
    fn friction_language_ratio_scores_55_just_below_10_percent_boundary() {
        // 1/14 = 7.14% — in [5.0, 10.0) → score 55.
        let snapshot = friction_commits(14, 1);
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 7.14).abs() < 0.1, "Expected ~7.14%, got {p}"),
            _ => panic!("Expected Percentage"),
        }
        assert_eq!(result.score, Some(55), "~7.1% friction should score 55");
    }

    #[test]
    fn friction_language_ratio_scores_35_just_below_20_percent_boundary() {
        // 3/20 = 15.0% — in [10.0, 20.0) → score 35.
        let snapshot = friction_commits(20, 3);
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 15.0).abs() < 0.1, "Expected ~15.0%, got {p}"),
            _ => panic!("Expected Percentage"),
        }
        assert_eq!(result.score, Some(35), "~15% friction should score 35");
    }

    #[test]
    fn friction_language_ratio_returns_na_when_no_commits_in_window() {
        let snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Text(ref s) => assert_eq!(s, "N/A"),
            _ => panic!("Expected Text(N/A) for empty commit list"),
        }
        assert_eq!(result.score, None);
    }

    #[test]
    fn friction_language_ratio_is_case_insensitive() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![Commit {
            id: CommitId(0),
            author: 0,
            timestamp: now - Duration::days(1),
            message: "HACK: SHIP IT ANYWAY".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        }];
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 100.0).abs() < 1.0),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn friction_language_ratio_ignores_hackathon_mentions() {
        // "hack" must match as a whole word, not as a substring of an
        // unrelated word like "hackathon" — a team that runs hackathons
        // should not have every such commit counted as technical-debt
        // friction.
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "prep slides for the hackathon".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "feat: add login".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => {
                assert!(
                    (p - 0.0).abs() < 1.0,
                    "hackathon must not count as friction, got {p}%"
                )
            }
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn firefighting_ratio_word_boundary_ignores_partial_matches() {
        // "hotfix" must match as a whole word, not as a substring of an
        // unrelated word (mirrors the hackathon fix on the friction side —
        // both metrics share the same word-boundary matching helper).
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "renamed HotfixManager to PatchManager".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "feat: add login".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        let result = firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!(
                (p - 0.0).abs() < 1.0,
                "HotfixManager must not count as firefighting, got {p}%"
            ),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn firefighting_ratio_matches_common_inflected_forms() {
        // The word-boundary fix must not lose common inflections the old
        // substring match used to catch — "reverting", "hotfixed" are
        // genuine occurrences of the keyword, unlike "HotfixManager".
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "reverting the bad migration".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "hotfixed the prod outage".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        let result = firefighting_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!(
                (p - 100.0).abs() < 1.0,
                "both inflected forms must count as firefighting, got {p}%"
            ),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn friction_language_ratio_matches_common_inflected_forms() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: "workarounds needed for flaky CI".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "hacked together a quick patch".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!(
                (p - 100.0).abs() < 1.0,
                "both inflected forms must count as friction, got {p}%"
            ),
            _ => panic!("Expected Percentage"),
        }
    }

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
    fn gitignore_does_not_flag_source_modules_with_sensitive_terminology() {
        let paths = [
            "src/infrastructure/crypto/root-secret.ts",
            "src/cli/secret.ts",
            "src/application/redact-secrets.ts",
            "src/infrastructure/auth/auth-secret.ts",
            "src/application/signing-secret.ts",
            "src/application/redact-secrets.test.ts",
        ];
        let result = gitignore_findings(&paths);
        assert!(matches!(result.raw_value, RawValue::Count(0)));
        assert_eq!(result.score, Some(100));
    }

    #[test]
    fn gitignore_directory_rules_outrank_source_extensions() {
        let result = gitignore_findings(&[
            "node_modules/package/index.ts",
            "src/__pycache__/generated.py",
        ]);
        match result.raw_value {
            RawValue::List(items) => assert_eq!(
                items,
                vec![
                    String::from("node_modules/package/index.ts"),
                    String::from("src/__pycache__/generated.py"),
                ]
            ),
            other => panic!("expected findings list, got {other:?}"),
        }
    }

    #[test]
    fn gitignore_exact_environment_and_extension_rules_remain_active() {
        let paths = [
            ".env",
            ".env.production",
            "certs/server.pem",
            "private/signing.key",
            ".DS_Store",
            "Thumbs.db",
            "cache/value.pyc",
            "config/credentials.json",
        ];
        let result = gitignore_findings(&paths);
        match result.raw_value {
            RawValue::List(items) => assert_eq!(items, paths.map(String::from).to_vec()),
            other => panic!("expected findings list, got {other:?}"),
        }
        assert_eq!(result.score, Some(20));
    }

    #[test]
    fn gitignore_safe_templates_and_documentation_are_not_findings() {
        let result = gitignore_findings(&[
            ".env.example",
            ".env.sample",
            ".env.template",
            "docs/secret-management.md",
            "docs/credentials.md",
        ]);
        assert!(matches!(result.raw_value, RawValue::Count(0)));
        assert_eq!(result.score, Some(100));
    }

    #[test]
    fn gitignore_semantic_matching_respects_filename_token_boundaries() {
        let result = gitignore_findings(&[
            "src/monkey.rs",
            "src/secretary.ts",
            "docs/credentials-overview.txt",
            "credential-service/config.yaml",
        ]);
        assert!(matches!(result.raw_value, RawValue::Count(0)));
        assert_eq!(result.score, Some(100));
    }
}
