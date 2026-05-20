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

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(args: &ContributorsArgs) -> Result<()> {
    let repo_path = std::path::PathBuf::from(&args.target);

    let time_window = if let Some(since_str) = &args.since {
        let now = Utc::now();
        let since = parse_time_spec(since_str, now);
        if since.is_some() {
            TimeWindow {
                since,
                until: Some(now),
                default_months: 0,
            }
        } else {
            TimeWindow::full_history()
        }
    } else {
        // No --since flag: use full history so we don't miss any alias.
        TimeWindow::full_history()
    };
    let collector = Collector::open(&repo_path, time_window)?;
    let repo_path = collector.repo_path().to_path_buf();

    let collection = collector.collect_commits()?;
    let authors = &collection.authors;
    let commits = &collection.commits;

    // Count commits per AuthorId.
    let commit_counts: HashMap<AuthorId, usize> =
        commits.iter().fold(HashMap::new(), |mut acc, commit| {
            *acc.entry(commit.author).or_insert(0) += 1;
            acc
        });

    let groups = detect_duplicates(authors, &commit_counts);

    if groups.is_empty() {
        println!("No suspected duplicates found.");
        return Ok(());
    }

    println!("Suspected duplicates:\n");

    // Collect suggested entries functionally — separated from the printing loop below.
    let all_entries: Vec<String> = groups
        .iter()
        .filter(|g| g.emails.len() >= 2)
        .flat_map(|g| {
            let canonical = &g.emails[0].0;
            g.emails
                .iter()
                .skip(1)
                .map(|(alias, _)| format_mailmap_entry(&g.canonical_name, canonical, alias))
        })
        .collect();

    for group in &groups {
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

    println!("Note: grouping is by display name only — verify suggestions before using --write.");
    if args.write {
        write_mailmap_entries(&repo_path, &all_entries)?;
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
        assert_eq!(content, existing, "file must be unchanged when all entries exist");
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
