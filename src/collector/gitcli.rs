use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Progress;
use crate::cache::blame::BlameCache;
use crate::snapshot::{Author, AuthorId, BlameLine, FileEntry};

/// Collect blame data for all non-binary files in parallel using git CLI.
pub fn collect_blame(
    repo_path: &Path,
    files: &[FileEntry],
    authors: &[Author],
    progress: &dyn Progress,
) -> Result<HashMap<PathBuf, Vec<BlameLine>>> {
    let email_to_id: HashMap<&str, AuthorId> =
        authors.iter().map(|a| (a.email.as_str(), a.id)).collect();

    let results: Vec<(PathBuf, Vec<BlameLine>)> = files
        .par_iter()
        .filter(|f| !f.is_binary)
        .filter_map(|f| {
            let result = match blame_file(repo_path, &f.path, &email_to_id) {
                Ok(lines) if !lines.is_empty() => Some((f.path.clone(), lines)),
                Ok(_) => None,
                Err(_) => None, // Skip files that fail to blame (e.g., submodules)
            };
            progress.inc(1);
            result
        })
        .collect();

    Ok(results.into_iter().collect())
}

/// Collect blame, reusing cached entries where blob OID matches.
pub fn collect_blame_cached(
    repo_path: &Path,
    files: &[FileEntry],
    authors: &[Author],
    cache: &BlameCache,
    progress: &dyn Progress,
) -> Result<(HashMap<PathBuf, Vec<BlameLine>>, BlameCache)> {
    let email_to_id: HashMap<&str, AuthorId> =
        authors.iter().map(|a| (a.email.as_str(), a.id)).collect();

    let results: Vec<(PathBuf, Vec<BlameLine>, String)> = files
        .par_iter()
        .filter(|f| !f.is_binary)
        .filter_map(|f| {
            let lines = if let Some(cached) = cache.entries.get(&f.blob_oid) {
                cached.clone()
            } else {
                match blame_file(repo_path, &f.path, &email_to_id) {
                    Ok(lines) => lines,
                    Err(_) => Vec::new(),
                }
            };
            progress.inc(1);
            if lines.is_empty() {
                None
            } else {
                Some((f.path.clone(), lines, f.blob_oid.clone()))
            }
        })
        .collect();

    let mut new_cache = BlameCache::default();
    let mut blame_map = HashMap::new();
    for (path, lines, oid) in results {
        new_cache.entries.insert(oid, lines.clone());
        blame_map.insert(path, lines);
    }

    Ok((blame_map, new_cache))
}

fn blame_file(
    repo_path: &Path,
    file_path: &Path,
    email_to_id: &HashMap<&str, AuthorId>,
) -> Result<Vec<BlameLine>> {
    let output = Command::new("git")
        .args(["blame", "--porcelain"])
        .arg(file_path.to_str().unwrap_or(""))
        .current_dir(repo_path)
        .output()
        .context("Failed to run git blame")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_porcelain_blame(&stdout, email_to_id)
}

fn parse_porcelain_blame(
    output: &str,
    email_to_id: &HashMap<&str, AuthorId>,
) -> Result<Vec<BlameLine>> {
    let mut lines = Vec::new();
    let mut current_commit: Option<String> = None;
    let mut current_email: Option<String> = None;
    let mut current_timestamp: Option<DateTime<Utc>> = None;

    for line in output.lines() {
        if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
            // This is a commit header line: <hash> <orig_line> <final_line> [<num_lines>]
            let hash = &line[..40];
            current_commit = Some(hash.to_string());
        } else if let Some(mail) = line.strip_prefix("author-mail <") {
            let email = mail.trim_end_matches('>').to_lowercase();
            current_email = Some(email);
        } else if let Some(time_str) = line.strip_prefix("author-time ") {
            if let Ok(ts) = time_str.parse::<i64>() {
                current_timestamp = Utc.timestamp_opt(ts, 0).single();
            }
        } else if line.starts_with('\t') {
            // This is the actual source line — finalize the blame entry
            if let (Some(commit_id), Some(email), Some(timestamp)) =
                (&current_commit, &current_email, &current_timestamp)
            {
                let author_id = email_to_id.get(email.as_str()).copied().unwrap_or(0); // Fall back to first author if unknown

                lines.push(BlameLine {
                    author_id,
                    commit_id: commit_id.clone(),
                    timestamp: *timestamp,
                });
            }
        }
    }

    Ok(lines)
}

/// Check if the repository is a shallow clone.
pub fn is_shallow_clone(repo_path: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(repo_path)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_porcelain_blame_extracts_lines() {
        let porcelain = "\
abc1234567890123456789012345678901234567 1 1 1
author Test Author
author-mail <test@example.com>
author-time 1700000000
author-tz +0000
committer Test Author
committer-mail <test@example.com>
committer-time 1700000000
committer-tz +0000
summary Test commit
filename test.rs
\tlet x = 1;
";
        let email_to_id: HashMap<&str, AuthorId> = [("test@example.com", 0)].into_iter().collect();

        let lines = parse_porcelain_blame(porcelain, &email_to_id).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].author_id, 0);
        assert_eq!(
            lines[0].commit_id,
            "abc1234567890123456789012345678901234567"
        );
    }

    #[test]
    fn parse_porcelain_blame_handles_multiple_lines() {
        let porcelain = "\
abc1234567890123456789012345678901234567 1 1 2
author Test
author-mail <a@b.com>
author-time 1700000000
author-tz +0000
committer Test
committer-mail <a@b.com>
committer-time 1700000000
committer-tz +0000
summary msg
filename f.rs
\tline 1
abc1234567890123456789012345678901234567 2 2
\tline 2
";
        let email_to_id: HashMap<&str, AuthorId> = [("a@b.com", 0)].into_iter().collect();

        let lines = parse_porcelain_blame(porcelain, &email_to_id).unwrap();
        assert_eq!(lines.len(), 2);
    }
}
