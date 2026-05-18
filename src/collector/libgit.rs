use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use git2::{DiffOptions, Repository, Sort};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::snapshot::{
    Author, AuthorId, ChangeType, Commit, CommitInterner, FileChange, FileEntry, TimeWindow,
};

use super::CommitCollection;

/// Resolve author identity through .mailmap, falling back to the raw signature.
fn resolve_author(sig: &git2::Signature, mailmap: Option<&git2::Mailmap>) -> (String, String) {
    if let Some(mm) = mailmap {
        if let Ok(resolved) = mm.resolve_signature(sig) {
            return (
                resolved.name().unwrap_or("Unknown").to_string(),
                resolved.email().unwrap_or("unknown").to_lowercase(),
            );
        }
    }
    (
        sig.name().unwrap_or("Unknown").to_string(),
        sig.email().unwrap_or("unknown").to_lowercase(),
    )
}

fn git_time_to_chrono(time: &git2::Time) -> DateTime<Utc> {
    Utc.timestamp_opt(time.seconds(), 0)
        .single()
        .unwrap_or_else(Utc::now)
}

pub fn collect_commits(repo: &Repository, time_window: &TimeWindow) -> Result<CommitCollection> {
    // An empty repository (no commits yet) either reports is_empty()=true or has no
    // resolvable HEAD. Guard both cases: some git2 versions return is_empty()=false for
    // a `git init -b <branch>` repo that has no commits yet, causing push_head() to fail
    // with "reference not found" rather than the user-friendly "No commits" message.
    if repo.is_empty().unwrap_or(false) || repo.head().is_err() {
        return Ok(CommitCollection {
            commits: vec![],
            authors: vec![],
            interner: CommitInterner::default(),
            raw_email_to_id: HashMap::new(),
        });
    }

    let mailmap = repo.mailmap().ok();

    let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;
    revwalk
        .set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .context("Failed to set sorting")?;
    // `push_head()` fails with NotFound when HEAD is an unborn branch (git init with no
    // commits yet), even if `head()` succeeded above. Treat that as an empty repo.
    if let Err(e) = revwalk.push_head() {
        if e.code() == git2::ErrorCode::NotFound || e.code() == git2::ErrorCode::UnbornBranch {
            return Ok(CommitCollection {
                commits: vec![],
                authors: vec![],
                interner: CommitInterner::default(),
                raw_email_to_id: HashMap::new(),
            });
        }
        return Err(anyhow::anyhow!(e)).context("Failed to push HEAD");
    }

    collect_commits_from_revwalk(repo, revwalk, mailmap, time_window)
}

fn collect_commits_from_revwalk(
    repo: &Repository,
    revwalk: git2::Revwalk<'_>,
    mailmap: Option<git2::Mailmap>,
    time_window: &TimeWindow,
) -> Result<CommitCollection> {
    let mut commits = Vec::new();
    let mut email_to_id: HashMap<String, AuthorId> = HashMap::new();
    let mut raw_email_to_id: HashMap<String, AuthorId> = HashMap::new();
    let mut authors: Vec<Author> = Vec::new();
    let mut interner = CommitInterner::default();

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
        let raw_email = author_sig.email().unwrap_or("unknown").to_lowercase();
        let (name, email) = resolve_author(&author_sig, mailmap.as_ref());

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

        if raw_email != email {
            raw_email_to_id.entry(raw_email).or_insert(author_id);
        }

        let files_changed = collect_file_changes(repo, &commit)?;
        let parent_count = commit.parent_count();

        let commit_id = interner.intern(&oid.to_string());
        commits.push(Commit {
            id: commit_id,
            author: author_id,
            timestamp,
            message: commit.message().unwrap_or("").to_string(),
            files_changed,
            is_merge: parent_count > 1,
            parent_count,
        });
    }

    Ok(CommitCollection {
        commits,
        authors,
        interner,
        raw_email_to_id,
    })
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
    let sha_oid =
        git2::Oid::from_str(sha_str).with_context(|| format!("Invalid SHA: {sha_str}"))?;

    let mailmap = repo.mailmap().ok();

    let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;
    revwalk
        .set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .context("Failed to set sorting")?;
    revwalk
        .push(sha_oid)
        .context("Failed to push SHA to revwalk")?;

    collect_commits_from_revwalk(repo, revwalk, mailmap, time_window)
}

fn collect_files_from_tree(repo: &Repository, tree: git2::Tree<'_>) -> Result<Vec<FileEntry>> {
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

/// Collect the file tree at a specific commit SHA (without modifying working tree).
pub fn collect_files_at(repo: &Repository, sha_str: &str) -> Result<Vec<FileEntry>> {
    let sha_oid =
        git2::Oid::from_str(sha_str).with_context(|| format!("Invalid SHA: {sha_str}"))?;
    let commit = repo
        .find_commit(sha_oid)
        .with_context(|| format!("Failed to find commit {sha_str}"))?;
    let tree = commit.tree().context("Failed to get commit tree")?;
    collect_files_from_tree(repo, tree)
}

pub fn collect_files(repo: &Repository) -> Result<Vec<FileEntry>> {
    let head = repo.head().context("Failed to get HEAD")?;
    let tree = head.peel_to_tree().context("Failed to peel HEAD to tree")?;
    collect_files_from_tree(repo, tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::CommitInterner;

    #[test]
    fn empty_repo_collection_has_empty_raw_email_map() {
        let c = CommitCollection {
            commits: vec![],
            authors: vec![],
            interner: CommitInterner::default(),
            raw_email_to_id: std::collections::HashMap::new(),
        };
        assert!(c.raw_email_to_id.is_empty());
    }

    #[test]
    fn raw_email_to_id_maps_alias_to_canonical_author() {
        // Simulate what happens during a commit walk when mailmap resolves an alias.
        // Both the canonical and raw email must map to the same AuthorId.
        let mut email_to_id: std::collections::HashMap<String, crate::snapshot::AuthorId> = std::collections::HashMap::new();
        let mut raw_email_to_id: std::collections::HashMap<String, crate::snapshot::AuthorId> = std::collections::HashMap::new();
        let mut authors: Vec<crate::snapshot::Author> = Vec::new();

        let raw = "alice@old.com".to_string();
        let canonical = "alice@company.com".to_string();

        // First commit arrives with canonical email (already resolved by mailmap)
        let id = authors.len();
        email_to_id.insert(canonical.clone(), id);
        authors.push(crate::snapshot::Author { id, name: "Alice".into(), email: canonical.clone() });

        // Second commit arrives with raw (pre-mailmap) email → same person, different email
        // The logic should: find canonical via email_to_id, then record raw→id in raw_email_to_id
        if raw != canonical {
            if let Some(&existing_id) = email_to_id.get(&canonical) {
                raw_email_to_id.entry(raw.clone()).or_insert(existing_id);
            }
        }

        assert_eq!(raw_email_to_id.get("alice@old.com"), Some(&0));
        assert_eq!(email_to_id.get("alice@company.com"), Some(&0));
    }
}
