use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;

use crate::cli::ContributorsArgs;
use crate::collector::Collector;
use crate::runner::parse_time_spec;
use crate::snapshot::{Author, AuthorId, TimeWindow};

// ── Public types ──────────────────────────────────────────────────────────────

pub struct DuplicateGroup {
    pub canonical_name: String,
    /// (email, commit_count) pairs — sorted descending by commit_count
    pub emails: Vec<(String, usize)>,
}

// ── Core logic ────────────────────────────────────────────────────────────────

/// Group authors by lowercase name; return groups with 2+ distinct emails.
/// Within each group the email with the most commits comes first, and its
/// author's display-name becomes `canonical_name`.
pub(crate) fn detect_duplicates(
    authors: &[Author],
    commit_counts: &HashMap<AuthorId, usize>,
) -> Vec<DuplicateGroup> {
    // name_lower → Vec<(author_id, email, display_name)>
    let mut by_name: HashMap<String, Vec<(AuthorId, String, String)>> = HashMap::new();
    for author in authors {
        by_name
            .entry(author.name.to_lowercase())
            .or_default()
            .push((author.id, author.email.clone(), author.name.clone()));
    }

    let mut groups: Vec<DuplicateGroup> = by_name
        .into_values()
        .filter(|entries| {
            // At least 2 distinct emails in this name group.
            let distinct_emails: std::collections::HashSet<&str> =
                entries.iter().map(|(_, email, _)| email.as_str()).collect();
            distinct_emails.len() >= 2
        })
        .map(|mut entries| {
            // Sort entries by commit count descending so the first entry is the canonical one.
            entries.sort_by(|(id_a, _, _), (id_b, _, _)| {
                let count_a = commit_counts.get(id_a).copied().unwrap_or(0);
                let count_b = commit_counts.get(id_b).copied().unwrap_or(0);
                count_b.cmp(&count_a)
            });

            let canonical_name = entries[0].2.clone();
            let emails = entries
                .iter()
                .map(|(id, email, _)| {
                    let count = commit_counts.get(id).copied().unwrap_or(0);
                    (email.clone(), count)
                })
                .collect();

            DuplicateGroup {
                canonical_name,
                emails,
            }
        })
        .collect();

    // Stable sort by canonical name for deterministic output.
    groups.sort_by(|a, b| a.canonical_name.cmp(&b.canonical_name));
    groups
}

/// Returns a `.mailmap` entry in the form:
/// `Name <canonical@email> <alias@email>`
pub(crate) fn format_mailmap_entry(name: &str, canonical_email: &str, alias_email: &str) -> String {
    format!("{} <{}> <{}>", name, canonical_email, alias_email)
}

/// Append new `.mailmap` entries to `<repo_path>/.mailmap`, skipping any that
/// are already present (exact line match).
pub(crate) fn write_mailmap_entries(repo_path: &Path, entries: &[String]) -> Result<()> {
    let mailmap_path = repo_path.join(".mailmap");
    let existing = std::fs::read_to_string(&mailmap_path).unwrap_or_default();
    let existing_lines: std::collections::HashSet<&str> = existing.lines().collect();

    let new_entries: Vec<&str> = entries
        .iter()
        .map(String::as_str)
        .filter(|entry| !existing_lines.contains(entry))
        .collect();

    if new_entries.is_empty() {
        return Ok(());
    }

    let suffix = new_entries.join("\n") + "\n";
    let content = if existing.is_empty() {
        suffix
    } else if existing.ends_with('\n') {
        existing + &suffix
    } else {
        existing + "\n" + &suffix
    };
    std::fs::write(&mailmap_path, content)?;
    Ok(())
}

/// The analysis window for alias detection: `--since` when it parses,
/// otherwise full history so no alias is missed.
fn resolve_time_window(since: Option<&str>) -> TimeWindow {
    let now = Utc::now();
    since
        .and_then(|s| parse_time_spec(s, now))
        .map(|since| TimeWindow {
            since: Some(since),
            until: Some(now),
            default_months: 0,
        })
        .unwrap_or_else(TimeWindow::full_history)
}

/// All suggested `.mailmap` lines across `groups`: one entry per alias
/// email, mapping it to the group's canonical (most-committed) email.
fn mailmap_suggestions(groups: &[DuplicateGroup]) -> Vec<String> {
    groups
        .iter()
        .filter(|g| g.emails.len() >= 2)
        .flat_map(|g| {
            let canonical = &g.emails[0].0;
            g.emails
                .iter()
                .skip(1)
                .map(|(alias, _)| format_mailmap_entry(&g.canonical_name, canonical, alias))
        })
        .collect()
}

/// Print one duplicate group: aligned email/commit rows plus its
/// suggested `.mailmap` entries.
fn print_group(group: &DuplicateGroup) {
    println!("  {}", group.canonical_name);

    let max_email_len = group.emails.iter().map(|(e, _)| e.len()).max().unwrap_or(0);
    for (email, count) in &group.emails {
        println!(
            "    {:<width$}  {} commits",
            email,
            count,
            width = max_email_len
        );
    }

    if group.emails.len() >= 2 {
        let canonical_email = &group.emails[0].0;
        println!("\n  Suggested .mailmap entries:");
        for (alias_email, _) in group.emails.iter().skip(1) {
            println!(
                "    {}",
                format_mailmap_entry(&group.canonical_name, canonical_email, alias_email)
            );
        }
    }

    println!();
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(args: &ContributorsArgs) -> Result<()> {
    let repo_path = std::path::PathBuf::from(&args.target);
    let time_window = resolve_time_window(args.since.as_deref());
    let collector = Collector::open(&repo_path, time_window)?;
    let repo_path = collector.repo_path().to_path_buf();

    let collection = collector.collect_commits()?;

    // Count commits per AuthorId.
    let commit_counts: HashMap<AuthorId, usize> =
        collection
            .commits
            .iter()
            .fold(HashMap::new(), |mut acc, commit| {
                *acc.entry(commit.author).or_insert(0) += 1;
                acc
            });

    let groups = detect_duplicates(&collection.authors, &commit_counts);

    if groups.is_empty() {
        println!("No suspected duplicates found.");
        return Ok(());
    }

    println!("Suspected duplicates:\n");
    groups.iter().for_each(print_group);

    println!("Note: grouping is by display name only — verify suggestions before using --write.");
    if args.write {
        write_mailmap_entries(&repo_path, &mailmap_suggestions(&groups))?;
        println!("Written to .mailmap.");
    } else {
        println!("Run with --write to append to .mailmap");
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_author(id: usize, name: &str, email: &str) -> Author {
        Author {
            id,
            name: name.to_string(),
            email: email.to_string(),
        }
    }

    fn counts(pairs: &[(usize, usize)]) -> HashMap<AuthorId, usize> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn no_duplicates_when_unique_emails_per_name() {
        let authors = vec![
            make_author(0, "Alice Smith", "alice@company.com"),
            make_author(1, "Bob Jones", "bob@company.com"),
        ];
        let commit_counts = counts(&[(0, 10), (1, 5)]);
        let groups = detect_duplicates(&authors, &commit_counts);
        assert!(groups.is_empty());
    }

    #[test]
    fn detects_same_name_different_emails() {
        let authors = vec![
            make_author(0, "Alice Smith", "alice@company.com"),
            make_author(1, "Alice Smith", "alice@old.com"),
        ];
        let commit_counts = counts(&[(0, 42), (1, 8)]);
        let groups = detect_duplicates(&authors, &commit_counts);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].canonical_name, "Alice Smith");
        assert_eq!(groups[0].emails.len(), 2);
    }

    #[test]
    fn canonical_email_is_the_one_with_most_commits() {
        let authors = vec![
            // alice@old.com has fewer commits but appears first in the slice
            make_author(0, "Alice Smith", "alice@old.com"),
            make_author(1, "Alice Smith", "alice@company.com"),
        ];
        let commit_counts = counts(&[(0, 8), (1, 42)]);
        let groups = detect_duplicates(&authors, &commit_counts);
        assert_eq!(groups.len(), 1);
        // The email with 42 commits should be first (canonical).
        assert_eq!(groups[0].emails[0].0, "alice@company.com");
        assert_eq!(groups[0].emails[0].1, 42);
        assert_eq!(groups[0].emails[1].0, "alice@old.com");
        assert_eq!(groups[0].emails[1].1, 8);
    }

    #[test]
    fn resolve_time_window_none_is_full_history() {
        let w = resolve_time_window(None);
        assert!(w.since.is_none() && w.until.is_none());
    }

    #[test]
    fn resolve_time_window_parses_relative_spec() {
        let w = resolve_time_window(Some("3months"));
        assert!(w.since.is_some() && w.until.is_some());
    }

    #[test]
    fn resolve_time_window_unparseable_falls_back_to_full_history() {
        let w = resolve_time_window(Some("not-a-date"));
        assert!(w.since.is_none() && w.until.is_none());
    }

    #[test]
    fn mailmap_suggestions_map_aliases_to_canonical() {
        let groups = vec![DuplicateGroup {
            canonical_name: "Alice Smith".into(),
            emails: vec![
                ("alice@company.com".into(), 42),
                ("alice@old.com".into(), 8),
                ("alice@older.com".into(), 1),
            ],
        }];
        assert_eq!(
            mailmap_suggestions(&groups),
            vec![
                "Alice Smith <alice@company.com> <alice@old.com>",
                "Alice Smith <alice@company.com> <alice@older.com>",
            ]
        );
    }

    #[test]
    fn format_mailmap_entry_produces_correct_string() {
        let entry = format_mailmap_entry("Alice Smith", "alice@company.com", "alice@old.com");
        assert_eq!(entry, "Alice Smith <alice@company.com> <alice@old.com>");
    }

    #[test]
    fn groups_authors_case_insensitively() {
        // "alice smith" and "Alice Smith" must be treated as the same person
        let authors = vec![
            Author {
                id: 0,
                name: "Alice Smith".into(),
                email: "alice@new.com".into(),
            },
            Author {
                id: 1,
                name: "alice smith".into(),
                email: "alice@old.com".into(),
            },
        ];
        let commit_counts = HashMap::from([(0, 10), (1, 2)]);
        let groups = detect_duplicates(&authors, &commit_counts);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].canonical_name, "Alice Smith"); // name from the author with most commits
    }

    #[test]
    fn run_with_write_maps_alias_to_most_committed_email() {
        // Two identities for one name: old@x authors 1 commit first, then
        // new@x authors 2 — the canonical email must be new@x (most
        // commits), which also pins the per-author commit counting.
        let dir = TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap()
                .success());
        };
        git(&["init", "-q"]);
        let commit_as = |email: &str, file: &str| {
            std::fs::write(dir.path().join(file), file).unwrap();
            git(&["add", "-A"]);
            git(&[
                "-c",
                &format!("user.email={email}"),
                "-c",
                "user.name=Alice Smith",
                "commit",
                "-q",
                "-m",
                "c",
            ]);
        };
        commit_as("old@x", "one.txt");
        commit_as("new@x", "two.txt");
        commit_as("new@x", "three.txt");

        let args = ContributorsArgs {
            target: dir.path().to_string_lossy().into_owned(),
            write: true,
            since: None,
        };
        run(&args).unwrap();

        let mailmap = std::fs::read_to_string(dir.path().join(".mailmap")).unwrap();
        assert!(
            mailmap.contains("Alice Smith <new@x> <old@x>"),
            "canonical must be the most-committed email; got: {mailmap:?}"
        );
    }

    #[test]
    fn write_mailmap_creates_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let entries = vec!["Alice Smith <alice@new.com> <alice@old.com>".to_string()];
        write_mailmap_entries(repo_path, &entries).unwrap();
        let content = std::fs::read_to_string(repo_path.join(".mailmap")).unwrap();
        assert!(content.contains("Alice Smith <alice@new.com> <alice@old.com>"));
    }

    #[test]
    fn write_mailmap_no_op_when_all_entries_already_exist() {
        let dir = TempDir::new().unwrap();
        let existing = "Alice Smith <alice@company.com> <alice@old.com>\n";
        std::fs::write(dir.path().join(".mailmap"), existing).unwrap();

        write_mailmap_entries(
            dir.path(),
            &["Alice Smith <alice@company.com> <alice@old.com>".to_string()],
        )
        .unwrap();

        let content = std::fs::read_to_string(dir.path().join(".mailmap")).unwrap();
        assert_eq!(
            content, existing,
            "file must be unchanged when all entries exist"
        );
    }

    #[test]
    fn write_mailmap_appends_on_new_line_when_no_trailing_newline() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".mailmap"), "# existing header").unwrap();

        write_mailmap_entries(
            dir.path(),
            &["Alice Smith <alice@company.com> <alice@old.com>".to_string()],
        )
        .unwrap();

        let content = std::fs::read_to_string(dir.path().join(".mailmap")).unwrap();
        assert!(
            content.contains("\nAlice Smith"),
            "new entry must start on its own line, got: {:?}",
            content
        );
    }

    #[test]
    fn write_mailmap_skips_existing_entries() {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        let existing = "Alice Smith <alice@company.com> <alice@old.com>\n";
        std::fs::write(repo_path.join(".mailmap"), existing).unwrap();

        let entries = vec![
            "Alice Smith <alice@company.com> <alice@old.com>".to_string(),
            "Bob Jones <bob@company.com> <bob@old.com>".to_string(),
        ];
        write_mailmap_entries(repo_path, &entries).unwrap();

        let content = std::fs::read_to_string(repo_path.join(".mailmap")).unwrap();
        // Existing entry should not be duplicated.
        assert_eq!(
            content
                .matches("Alice Smith <alice@company.com> <alice@old.com>")
                .count(),
            1
        );
        // New entry should be added.
        assert!(content.contains("Bob Jones <bob@company.com> <bob@old.com>"));
    }
}
