use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use git2::{DiffOptions, Repository, Sort};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::snapshot::{Author, AuthorId, ChangeType, Commit, FileChange, FileEntry, TimeWindow};

use super::CommitCollection;

fn git_time_to_chrono(time: &git2::Time) -> DateTime<Utc> {
    Utc.timestamp_opt(time.seconds(), 0)
        .single()
        .unwrap_or_else(Utc::now)
}

pub fn collect_commits(repo: &Repository, time_window: &TimeWindow) -> Result<CommitCollection> {
    // An empty repository (no commits yet) has no HEAD reference.
    // Return an empty collection rather than propagating a confusing libgit2 error.
    if repo.is_empty().unwrap_or(false) {
        return Ok(CommitCollection {
            commits: vec![],
            authors: vec![],
        });
    }

    let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;
    revwalk
        .set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .context("Failed to set sorting")?;
    revwalk.push_head().context("Failed to push HEAD")?;

    let mut commits = Vec::new();
    let mut email_to_id: HashMap<String, AuthorId> = HashMap::new();
    let mut authors: Vec<Author> = Vec::new();

    for oid_result in revwalk {
        let oid = oid_result.context("Failed to get commit oid")?;
        let commit = repo.find_commit(oid).context("Failed to find commit")?;

        let timestamp = git_time_to_chrono(&commit.time());

        // Filter by time window
        if !time_window.contains(&timestamp) {
            // If commit is before the window start, we can stop
            // (commits are sorted by time descending)
            if let Some(since) = &time_window.since {
                if &timestamp < since {
                    break;
                }
            }
            continue;
        }

        // Deduplicate author by email
        let author_sig = commit.author();
        let email = author_sig.email().unwrap_or("unknown").to_lowercase();
        let name = author_sig.name().unwrap_or("Unknown").to_string();

        let author_id = if let Some(&id) = email_to_id.get(&email) {
            id
        } else {
            let id = authors.len();
            email_to_id.insert(email.clone(), id);
            authors.push(Author {
                id,
                name,
                email: email.clone(),
            });
            id
        };

        // Compute file changes by diffing against parent
        let files_changed = collect_file_changes(repo, &commit)?;
        let parent_count = commit.parent_count();

        commits.push(Commit {
            id: oid.to_string(),
            author: author_id,
            timestamp,
            message: commit.message().unwrap_or("").to_string(),
            files_changed,
            is_merge: parent_count > 1,
            parent_count,
        });
    }

    Ok(CommitCollection { commits, authors })
}

fn collect_file_changes(repo: &Repository, commit: &git2::Commit) -> Result<Vec<FileChange>> {
    let tree = commit.tree().context("Failed to get commit tree")?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(
            commit
                .parent(0)
                .context("Failed to get parent")?
                .tree()
                .context("Failed to get parent tree")?,
        )
    } else {
        None
    };

    let mut diff_opts = DiffOptions::new();
    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
        .context("Failed to create diff")?;

    let mut changes = Vec::new();

    for delta in diff.deltas() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .unwrap_or_else(|| std::path::Path::new("unknown"))
            .to_path_buf();

        let change_type = match delta.status() {
            git2::Delta::Added => ChangeType::Added,
            git2::Delta::Deleted => ChangeType::Deleted,
            git2::Delta::Modified => ChangeType::Modified,
            git2::Delta::Renamed => ChangeType::Renamed,
            _ => ChangeType::Modified,
        };

        changes.push(FileChange {
            path,
            additions: 0, // We'll compute stats below if needed
            deletions: 0,
            change_type,
        });
    }

    // Get line-level stats
    if let Ok(stats) = diff.stats() {
        // Stats are aggregate; distribute evenly as approximation
        // For more precision we'd need per-file stats via diff.foreach
        let _insertions = stats.insertions();
        let _deletions = stats.deletions();
    }

    // Get per-file stats using foreach
    let changes_clone = changes.clone();
    let mut file_stats: HashMap<PathBuf, (u32, u32)> = HashMap::new();

    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |delta, _hunk, line| {
            if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
                let entry = file_stats.entry(path.to_path_buf()).or_insert((0, 0));
                match line.origin() {
                    '+' => entry.0 += 1,
                    '-' => entry.1 += 1,
                    _ => {}
                }
            }
            true
        }),
    )
    .ok(); // Ignore errors in line-level stats

    let changes: Vec<FileChange> = changes_clone
        .into_iter()
        .map(|mut c| {
            if let Some(&(adds, dels)) = file_stats.get(&c.path) {
                c.additions = adds;
                c.deletions = dels;
            }
            c
        })
        .collect();

    Ok(changes)
}

/// Collect commits reachable from the given SHA (as if it were HEAD).
pub fn collect_commits_at(
    repo: &Repository,
    sha_str: &str,
    time_window: &TimeWindow,
) -> Result<CommitCollection> {
    let sha_oid = git2::Oid::from_str(sha_str)
        .with_context(|| format!("Invalid SHA: {sha_str}"))?;

    let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;
    revwalk
        .set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .context("Failed to set sorting")?;
    revwalk.push(sha_oid).context("Failed to push SHA to revwalk")?;

    let mut commits = Vec::new();
    let mut email_to_id: HashMap<String, AuthorId> = HashMap::new();
    let mut authors: Vec<Author> = Vec::new();

    for oid_result in revwalk {
        let oid = oid_result.context("Failed to get commit oid")?;
        let commit = repo.find_commit(oid).context("Failed to find commit")?;

        let timestamp = git_time_to_chrono(&commit.time());

        if !time_window.contains(&timestamp) {
            if let Some(since) = &time_window.since {
                if &timestamp < since {
                    break;
                }
            }
            continue;
        }

        let author_sig = commit.author();
        let email = author_sig.email().unwrap_or("unknown").to_lowercase();
        let name = author_sig.name().unwrap_or("Unknown").to_string();

        let author_id = if let Some(&id) = email_to_id.get(&email) {
            id
        } else {
            let id = authors.len();
            email_to_id.insert(email.clone(), id);
            authors.push(Author { id, name, email });
            id
        };

        let files_changed = collect_file_changes(repo, &commit)?;
        let parent_count = commit.parent_count();

        commits.push(Commit {
            id: oid.to_string(),
            author: author_id,
            timestamp,
            message: commit.message().unwrap_or("").to_string(),
            files_changed,
            is_merge: parent_count > 1,
            parent_count,
        });
    }

    Ok(CommitCollection { commits, authors })
}

/// Collect the file tree at a specific commit SHA (without modifying working tree).
pub fn collect_files_at(repo: &Repository, sha_str: &str) -> Result<Vec<FileEntry>> {
    let sha_oid = git2::Oid::from_str(sha_str)
        .with_context(|| format!("Invalid SHA: {sha_str}"))?;
    let commit = repo.find_commit(sha_oid)
        .with_context(|| format!("Failed to find commit {sha_str}"))?;
    let tree = commit.tree().context("Failed to get commit tree")?;

    let mut files = Vec::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if entry.kind() == Some(git2::ObjectType::Blob) {
            let name = entry.name().unwrap_or("");
            let path = if dir.is_empty() {
                PathBuf::from(name)
            } else {
                PathBuf::from(format!("{}{}", dir, name))
            };

            let depth = path.components().count();

            let is_binary = entry
                .to_object(repo)
                .ok()
                .and_then(|obj| obj.as_blob().map(|b| b.is_binary()))
                .unwrap_or(false);

            let size_bytes = entry
                .to_object(repo)
                .ok()
                .and_then(|obj| obj.as_blob().map(|b| b.size()))
                .unwrap_or(0) as u64;

            files.push(FileEntry {
                path,
                size_bytes,
                is_binary,
                depth,
                blob_oid: entry.id().to_string(),
            });
        }
        git2::TreeWalkResult::Ok
    })
    .context("Failed to walk tree")?;

    Ok(files)
}

pub fn collect_files(repo: &Repository) -> Result<Vec<FileEntry>> {
    let head = repo.head().context("Failed to get HEAD")?;
    let tree = head.peel_to_tree().context("Failed to peel HEAD to tree")?;

    let mut files = Vec::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if entry.kind() == Some(git2::ObjectType::Blob) {
            let name = entry.name().unwrap_or("");
            let path = if dir.is_empty() {
                PathBuf::from(name)
            } else {
                PathBuf::from(format!("{}{}", dir, name))
            };

            let depth = path.components().count();

            let is_binary = entry
                .to_object(repo)
                .ok()
                .and_then(|obj| obj.as_blob().map(|b| b.is_binary()))
                .unwrap_or(false);

            let size_bytes = entry
                .to_object(repo)
                .ok()
                .and_then(|obj| obj.as_blob().map(|b| b.size()))
                .unwrap_or(0) as u64;

            files.push(FileEntry {
                path,
                size_bytes,
                is_binary,
                depth,
                blob_oid: entry.id().to_string(),
            });
        }
        git2::TreeWalkResult::Ok
    })
    .context("Failed to walk tree")?;

    Ok(files)
}
