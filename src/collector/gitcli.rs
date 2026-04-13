use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Progress;
use crate::cache::blame::BlameCache;
use crate::snapshot::{compress_blame, Author, AuthorId, BlameLine, FileEntry};

fn build_email_map(authors: &[Author]) -> HashMap<&str, AuthorId> {
    authors.iter().map(|a| (a.email.as_str(), a.id)).collect()
}

/// Collect blame data for all non-binary files in parallel using git CLI.
pub fn collect_blame(
    repo_path: &Path,
    files: &[FileEntry],
    authors: &[Author],
    progress: &dyn Progress,
) -> Result<HashMap<PathBuf, Vec<BlameLine>>> {
    let (map, _) =
        collect_blame_cached(repo_path, files, authors, &BlameCache::default(), progress)?;
    Ok(map)
}

/// Collect blame, reusing cached entries where blob OID matches.
pub fn collect_blame_cached(
    repo_path: &Path,
    files: &[FileEntry],
    authors: &[Author],
    cache: &BlameCache,
    progress: &dyn Progress,
) -> Result<(HashMap<PathBuf, Vec<BlameLine>>, BlameCache)> {
    let email_to_id = build_email_map(authors);

    let results: Vec<(PathBuf, Vec<BlameLine>, String)> = files
        .par_iter()
        .filter(|f| !f.is_binary)
        .filter_map(|f| {
            let lines = if let Some(cached) = cache.entries.get(&f.blob_oid) {
                cached.clone()
            } else {
                blame_file(repo_path, &f.path, &email_to_id, None).unwrap_or_default()
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
        blame_map.insert(path, compress_blame(lines));
    }

    Ok((blame_map, new_cache))
}

fn blame_file(
    repo_path: &Path,
    file_path: &Path,
    email_to_id: &HashMap<&str, AuthorId>,
    at_rev: Option<&str>,
) -> Result<Vec<BlameLine>> {
    let mut cmd = Command::new("git");
    cmd.args(["blame", "--porcelain"]);
    if let Some(sha) = at_rev {
        cmd.arg(sha);
    }
    cmd.arg("--");
    let output = cmd
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

struct BlameParserState<'a> {
    email_to_id: &'a HashMap<&'a str, AuthorId>,
    current_email: Option<String>,
    current_timestamp: Option<DateTime<Utc>>,
    lines: Vec<BlameLine>,
}

impl<'a> BlameParserState<'a> {
    fn new(email_to_id: &'a HashMap<&'a str, AuthorId>) -> Self {
        Self {
            email_to_id,
            current_email: None,
            current_timestamp: None,
            lines: Vec::new(),
        }
    }

    fn process_line(&mut self, line: &str) {
        if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
            // commit header line — no fields to extract
        } else if let Some(mail) = line.strip_prefix("author-mail <") {
            self.current_email = Some(mail.trim_end_matches('>').to_lowercase());
        } else if let Some(time_str) = line.strip_prefix("author-time ") {
            if let Ok(ts) = time_str.parse::<i64>() {
                self.current_timestamp = Utc.timestamp_opt(ts, 0).single();
            }
        } else if line.starts_with('\t') {
            // actual source line — finalize the blame entry
            if let (Some(email), Some(timestamp)) = (&self.current_email, &self.current_timestamp) {
                let author_id = self.email_to_id.get(email.as_str()).copied().unwrap_or(0);
                self.lines.push(BlameLine {
                    author_id,
                    timestamp: *timestamp,
                    line_count: 1,
                });
            }
        }
    }

    fn finish(self) -> Vec<BlameLine> {
        self.lines
    }
}

fn parse_porcelain_blame(
    output: &str,
    email_to_id: &HashMap<&str, AuthorId>,
) -> Result<Vec<BlameLine>> {
    let mut state = BlameParserState::new(email_to_id);
    for line in output.lines() {
        state.process_line(line);
    }
    Ok(state.finish())
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

    // --- BlameParserState / parse_porcelain_blame ---

    #[test]
    fn parse_porcelain_blame_unknown_email_falls_back_to_author_zero() {
        let porcelain = "\
abc1234567890123456789012345678901234567 1 1 1
author Unknown
author-mail <nobody@nowhere.com>
author-time 1700000000
author-tz +0000
committer Unknown
committer-mail <nobody@nowhere.com>
committer-time 1700000000
committer-tz +0000
summary msg
filename f.rs
\tcode line
";
        // email_to_id is empty — unknown email should fall back to id 0
        let email_to_id: HashMap<&str, AuthorId> = HashMap::new();
        let lines = parse_porcelain_blame(porcelain, &email_to_id).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].author_id, 0);
    }

    #[test]
    fn parse_porcelain_blame_tab_line_without_author_emits_nothing() {
        // A tab line that arrives before any author-mail should not emit a blame entry
        let porcelain = "\
abc1234567890123456789012345678901234567 1 1 1
\torphan line
";
        let email_to_id: HashMap<&str, AuthorId> = HashMap::new();
        let lines = parse_porcelain_blame(porcelain, &email_to_id).unwrap();
        assert!(lines.is_empty(), "no entry without preceding author info");
    }

    #[test]
    fn parse_porcelain_blame_two_authors_attributed_correctly() {
        let porcelain = "\
aaaa234567890123456789012345678901234567 1 1 1
author Alice
author-mail <alice@example.com>
author-time 1700000000
author-tz +0000
committer Alice
committer-mail <alice@example.com>
committer-time 1700000000
committer-tz +0000
summary Alice's commit
filename f.rs
\talice line
bbbb234567890123456789012345678901234567 2 2 1
author Bob
author-mail <bob@example.com>
author-time 1700000001
author-tz +0000
committer Bob
committer-mail <bob@example.com>
committer-time 1700000001
committer-tz +0000
summary Bob's commit
filename f.rs
\tbob line
";
        let email_to_id: HashMap<&str, AuthorId> =
            [("alice@example.com", 0), ("bob@example.com", 1)]
                .into_iter()
                .collect();
        let lines = parse_porcelain_blame(porcelain, &email_to_id).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].author_id, 0);
        assert_eq!(lines[1].author_id, 1);
    }

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
