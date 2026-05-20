# Contributor Deduplication Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `.mailmap` respected throughout the entire pipeline (commits + blame), and add a `barad-dur contributors` subcommand that detects suspected duplicates and generates `.mailmap` entries.

**Architecture:** Two independent parts. Part 1 fixes a silent bug: blame parsing uses raw pre-mailmap emails, which fail to match the mailmap-resolved author list, silently attributing lines to author 0. Fix: capture the raw email alongside the resolved one during the commit walk and pass a reverse map to the blame parser. Part 2 adds a new read-only subcommand that groups commit authors by display name, flags multi-email groups as suspected duplicates, and optionally writes `.mailmap` entries.

**Tech Stack:** Rust, git2, clap, `std::collections::HashMap`

---

### Task 1: Add `raw_email_to_id` to `CommitCollection`

**Files:**
- Modify: `src/collector/types.rs`
- Modify: `src/collector/libgit.rs:41-45` and `58-63` (two empty-repo early returns)

**Step 1: Write the failing test**

In `src/collector/libgit.rs`, add inside the existing `#[cfg(test)]` block (or create one):

```rust
#[test]
fn empty_repo_collection_has_empty_raw_email_map() {
    // CommitCollection must always initialise raw_email_to_id
    let c = CommitCollection {
        commits: vec![],
        authors: vec![],
        interner: CommitInterner::default(),
        raw_email_to_id: std::collections::HashMap::new(),
    };
    assert!(c.raw_email_to_id.is_empty());
}
```

**Step 2: Run test to verify it fails**

```bash
rtk cargo test empty_repo_collection_has_empty_raw_email_map
```

Expected: compile error — field `raw_email_to_id` unknown in `CommitCollection`

**Step 3: Add the field**

In `src/collector/types.rs`:

```rust
use std::collections::HashMap;
use crate::snapshot::{Author, AuthorId, Commit, CommitInterner};

/// Result of collecting commits — includes deduplicated author list.
#[non_exhaustive]
pub struct CommitCollection {
    pub commits: Vec<Commit>,
    pub authors: Vec<Author>,
    pub interner: CommitInterner,
    /// Maps raw (pre-mailmap) email → AuthorId for blame resolution.
    pub raw_email_to_id: HashMap<String, AuthorId>,
}
```

Update the two early-return sites in `src/collector/libgit.rs` (lines ~41-45 and ~58-63):

```rust
return Ok(CommitCollection {
    commits: vec![],
    authors: vec![],
    interner: CommitInterner::default(),
    raw_email_to_id: HashMap::new(),
});
```

**Step 4: Run test to verify it passes**

```bash
rtk cargo test empty_repo_collection_has_empty_raw_email_map
```

Expected: PASS

**Step 5: Commit**

```bash
rtk git add src/collector/types.rs src/collector/libgit.rs
rtk git commit -m "feat(collector): add raw_email_to_id field to CommitCollection"
```

---

### Task 2: Populate `raw_email_to_id` during commit traversal

**Files:**
- Modify: `src/collector/libgit.rs:70-132` (`collect_commits_from_revwalk`)

**Step 1: Write the failing test**

Add to `src/collector/libgit.rs` tests:

```rust
#[test]
fn raw_email_to_id_captures_pre_mailmap_email() {
    // When resolved and raw email differ, both should map to the same AuthorId.
    // We test the logic directly: simulate two commits by the same person under
    // different emails resolving to the same canonical AuthorId.
    let mut raw_email_to_id: HashMap<String, AuthorId> = HashMap::new();
    let mut email_to_id: HashMap<String, AuthorId> = HashMap::new();
    let mut authors: Vec<Author> = Vec::new();

    let raw = "alice@old.com".to_string();
    let canonical = "alice@company.com".to_string();

    // First commit: canonical email
    let id = authors.len();
    email_to_id.insert(canonical.clone(), id);
    authors.push(Author { id, name: "Alice".into(), email: canonical.clone() });

    // Second commit: raw (pre-mailmap) email resolves to same author
    if !email_to_id.contains_key(&raw) {
        if let Some(&existing_id) = email_to_id.get(&canonical) {
            raw_email_to_id.insert(raw.clone(), existing_id);
        }
    }

    assert_eq!(raw_email_to_id.get("alice@old.com"), Some(&0));
}
```

**Step 2: Run test to verify it fails**

```bash
rtk cargo test raw_email_to_id_captures_pre_mailmap_email
```

Expected: compile error — `raw_email_to_id` not in scope (the logic isn't wired yet)

**Step 3: Implement in `collect_commits_from_revwalk`**

In `src/collector/libgit.rs`, update the function signature locals and author registration block (around line 76-130):

```rust
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
        // ... existing timestamp/window logic unchanged ...

        let author_sig = commit.author();
        // Capture raw email BEFORE mailmap resolution
        let raw_email = author_sig
            .email()
            .unwrap_or("unknown")
            .to_lowercase();

        let (name, email) = resolve_author(&author_sig, mailmap.as_ref());

        let author_id = if let Some(&id) = email_to_id.get(&email) {
            id
        } else {
            let id = authors.len();
            email_to_id.insert(email.clone(), id);
            authors.push(Author { id, name, email: email.clone() });
            id
        };

        // Record raw → canonical mapping when they differ
        if raw_email != email {
            raw_email_to_id.entry(raw_email).or_insert(author_id);
        }

        // ... rest of commit building unchanged ...
    }

    Ok(CommitCollection {
        commits,
        authors,
        interner,
        raw_email_to_id,
    })
}
```

**Step 4: Run tests**

```bash
rtk cargo test
```

Expected: all pass, no warnings

**Step 5: Commit**

```bash
rtk git add src/collector/libgit.rs
rtk git commit -m "feat(collector): populate raw_email_to_id from pre-mailmap emails"
```

---

### Task 3: Thread `raw_email_to_id` into blame parsing

**Files:**
- Modify: `src/collector/gitcli.rs` (functions `collect_blame`, `collect_blame_cached`, `blame_file`, `BlameParserState`)
- Modify: `src/collector/snapshot_builder.rs:178-183`

**Step 1: Write the failing test**

Add to `src/collector/gitcli.rs` tests:

```rust
#[test]
fn blame_parser_resolves_raw_email_via_reverse_map() {
    use std::collections::HashMap;
    use crate::snapshot::AuthorId;

    // canonical map has only the resolved email
    let mut email_to_id: HashMap<&str, AuthorId> = HashMap::new();
    email_to_id.insert("alice@company.com", 0);

    // raw map knows about the old email
    let mut raw_email_to_id: HashMap<String, AuthorId> = HashMap::new();
    raw_email_to_id.insert("alice@old.com".to_string(), 0);

    let porcelain = "\
abc1234567890123456789012345678901234567890 1 1 1\n\
author Alice\n\
author-mail <alice@old.com>\n\
author-time 1700000000\n\
\tsome code\n";

    let lines = parse_porcelain_blame(porcelain, &email_to_id, &raw_email_to_id).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].author_id, 0);
}
```

**Step 2: Run test to verify it fails**

```bash
rtk cargo test blame_parser_resolves_raw_email_via_reverse_map
```

Expected: compile error — `parse_porcelain_blame` wrong arity

**Step 3: Update `gitcli.rs`**

Thread `raw_email_to_id: &HashMap<String, AuthorId>` through the call chain:

```rust
// build_email_map unchanged

pub fn collect_blame(
    repo_path: &Path,
    files: &[FileEntry],
    authors: &[Author],
    raw_email_to_id: &HashMap<String, AuthorId>,
    progress: &dyn Progress,
) -> Result<HashMap<PathBuf, Vec<BlameLine>>> {
    let (map, _) = collect_blame_cached(
        repo_path, files, authors, raw_email_to_id, &BlameCache::default(), progress,
    )?;
    Ok(map)
}

pub fn collect_blame_cached(
    repo_path: &Path,
    files: &[FileEntry],
    authors: &[Author],
    raw_email_to_id: &HashMap<String, AuthorId>,
    cache: &BlameCache,
    progress: &dyn Progress,
) -> Result<(HashMap<PathBuf, Vec<BlameLine>>, BlameCache)> {
    let email_to_id = build_email_map(authors);

    let results: Vec<_> = files
        .par_iter()
        .filter(|f| !f.is_binary)
        .filter_map(|f| {
            let lines = if let Some(cached) = cache.entries.get(&f.blob_oid) {
                cached.clone()
            } else {
                blame_file(repo_path, &f.path, &email_to_id, raw_email_to_id, None)
                    .unwrap_or_default()
            };
            progress.inc(1);
            if lines.is_empty() { None } else { Some((f.path.clone(), lines, f.blob_oid.clone())) }
        })
        .collect();
    // ... rest unchanged ...
}

fn blame_file(
    repo_path: &Path,
    file_path: &Path,
    email_to_id: &HashMap<&str, AuthorId>,
    raw_email_to_id: &HashMap<String, AuthorId>,
    at_rev: Option<&str>,
) -> Result<Vec<BlameLine>> {
    // ... git blame invocation unchanged ...
    parse_porcelain_blame(&stdout, email_to_id, raw_email_to_id)
}
```

Update `BlameParserState` to carry `raw_email_to_id` and resolve in `process_line`:

```rust
struct BlameParserState<'a> {
    email_to_id: &'a HashMap<&'a str, AuthorId>,
    raw_email_to_id: &'a HashMap<String, AuthorId>,
    current_email: Option<String>,
    current_timestamp: Option<DateTime<Utc>>,
    lines: Vec<BlameLine>,
}

impl<'a> BlameParserState<'a> {
    fn new(
        email_to_id: &'a HashMap<&'a str, AuthorId>,
        raw_email_to_id: &'a HashMap<String, AuthorId>,
    ) -> Self {
        Self { email_to_id, raw_email_to_id, current_email: None, current_timestamp: None, lines: Vec::new() }
    }

    fn process_line(&mut self, line: &str) {
        // ... commit header and author-time unchanged ...
        } else if line.starts_with('\t') {
            if let (Some(email), Some(timestamp)) = (&self.current_email, &self.current_timestamp) {
                let author_id = self.email_to_id.get(email.as_str())
                    .or_else(|| self.raw_email_to_id.get(email.as_str()))
                    .copied()
                    .unwrap_or(0);
                self.lines.push(BlameLine { author_id, timestamp: *timestamp, line_count: 1 });
            }
        }
    }
}

fn parse_porcelain_blame(
    output: &str,
    email_to_id: &HashMap<&str, AuthorId>,
    raw_email_to_id: &HashMap<String, AuthorId>,
) -> Result<Vec<BlameLine>> {
    let mut state = BlameParserState::new(email_to_id, raw_email_to_id);
    // ... rest unchanged ...
}
```

Update `snapshot_builder.rs:178-183` to pass the new map:

```rust
let (map, mut updated_cache) = self.collect_blame_cached(
    &blame_files,
    &collection.authors,
    &collection.raw_email_to_id,   // <-- add this
    &blame_cache,
    blame_progress,
)?;
```

**Step 4: Run all tests**

```bash
rtk cargo test
```

Expected: all pass

**Step 5: Commit**

```bash
rtk git add src/collector/gitcli.rs src/collector/snapshot_builder.rs
rtk git commit -m "fix(blame): resolve pre-mailmap emails via raw_email_to_id"
```

---

### Task 4: Add `contributors` CLI subcommand

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

**Step 1: Add variant to `Commands` enum in `src/cli.rs`**

```rust
/// Detect suspected duplicate contributors and suggest .mailmap entries
Contributors(ContributorsArgs),
```

Add the args struct after the other `*Args` structs:

```rust
#[derive(clap::Args, Debug)]
#[command(about = "Detect suspected duplicate contributors and suggest .mailmap entries")]
pub struct ContributorsArgs {
    /// Path to the git repository
    #[arg(default_value = ".")]
    pub target: String,

    /// Append suggested entries to .mailmap (creates it if absent)
    #[arg(long)]
    pub write: bool,

    /// Only show authors with commits in this window (e.g. 6months, 1year)
    #[arg(long)]
    pub since: Option<String>,
}
```

**Step 2: Dispatch in `src/main.rs`**

Find the `match cli.command` block and add:

```rust
Commands::Contributors(args) => {
    contributors::run(&args)?;
}
```

Add `mod contributors;` near the top with the other module declarations.

**Step 3: Run test to verify it compiles**

```bash
rtk cargo build 2>&1 | head -20
```

Expected: error — module `contributors` not found (module file missing)

**Step 4: Commit the CLI skeleton**

```bash
rtk git add src/cli.rs src/main.rs
rtk git commit -m "feat(cli): add contributors subcommand skeleton"
```

---

### Task 5: Implement `src/contributors.rs`

**Files:**
- Create: `src/contributors.rs`

**Step 1: Write failing tests first**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::Author;

    fn make_author(id: usize, name: &str, email: &str) -> Author {
        Author { id, name: name.into(), email: email.into() }
    }

    #[test]
    fn no_duplicates_when_all_emails_unique_per_name() {
        let authors = vec![
            make_author(0, "alice", "alice@a.com"),
            make_author(1, "bob",   "bob@a.com"),
        ];
        let groups = detect_duplicates(&authors);
        assert!(groups.is_empty());
    }

    #[test]
    fn detects_same_name_different_emails() {
        let authors = vec![
            make_author(0, "Alice Smith", "alice@company.com"),
            make_author(1, "alice smith", "alice@old.com"),
        ];
        let groups = detect_duplicates(&authors);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].emails.len(), 2);
    }

    #[test]
    fn mailmap_entry_puts_most_recent_email_first() {
        // canonical = email with highest commit count (passed separately)
        let entry = format_mailmap_entry("Alice Smith", "alice@company.com", "alice@old.com");
        assert_eq!(entry, "Alice Smith <alice@company.com> <alice@old.com>");
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
rtk cargo test --lib contributors
```

Expected: compile error — module `contributors` not found

**Step 3: Implement `src/contributors.rs`**

```rust
use anyhow::Result;
use std::collections::HashMap;

use crate::cli::ContributorsArgs;
use crate::collector::Collector;
use crate::snapshot::{Author, TimeWindow};

pub struct DuplicateGroup {
    pub canonical_name: String,
    /// (email, commit_count) — sorted by commit_count descending
    pub emails: Vec<(String, usize)>,
}

/// Group authors by display name (case-insensitive). Groups with >1 email are duplicates.
pub fn detect_duplicates(authors: &[Author]) -> Vec<DuplicateGroup> {
    let mut by_name: HashMap<String, Vec<&Author>> = HashMap::new();
    for author in authors {
        by_name
            .entry(author.name.to_lowercase())
            .or_default()
            .push(author);
    }
    by_name
        .into_iter()
        .filter(|(_, members)| {
            let unique_emails: std::collections::HashSet<&str> =
                members.iter().map(|a| a.email.as_str()).collect();
            unique_emails.len() > 1
        })
        .map(|(_, members)| {
            // Use the name with the most commits as the canonical display name
            let canonical_name = members[0].name.clone();
            let emails = members
                .iter()
                .map(|a| (a.email.clone(), 0usize)) // commit counts filled in by caller
                .collect();
            DuplicateGroup { canonical_name, emails }
        })
        .collect()
}

pub fn format_mailmap_entry(name: &str, canonical_email: &str, alias_email: &str) -> String {
    format!("{} <{}> <{}>", name, canonical_email, alias_email)
}

pub fn run(args: &ContributorsArgs) -> Result<()> {
    let repo_path = crate::collector::resolve_target(&args.target)?;
    let collector = Collector::new(&repo_path);
    let time_window = TimeWindow::from_since(args.since.as_deref())?;
    let collection = collector.collect_commits_windowed(&time_window)?;

    // Count commits per author
    let mut commit_counts: HashMap<usize, usize> = HashMap::new();
    for commit in &collection.commits {
        *commit_counts.entry(commit.author).or_insert(0) += 1;
    }

    let mut groups = detect_duplicates(&collection.authors);

    if groups.is_empty() {
        println!("No suspected duplicates found.");
        return Ok(());
    }

    // Enrich groups with commit counts, sort emails by count descending
    for group in &mut groups {
        for (email, count) in &mut group.emails {
            if let Some(author) = collection.authors.iter().find(|a| &a.email == email) {
                *count = *commit_counts.get(&author.id).unwrap_or(&0);
            }
        }
        group.emails.sort_by(|a, b| b.1.cmp(&a.1));
    }

    // Sort groups by canonical name
    groups.sort_by(|a, b| a.canonical_name.cmp(&b.canonical_name));

    println!("Suspected duplicates:\n");
    let mut entries_to_write: Vec<String> = Vec::new();

    for group in &groups {
        println!("  {}", group.canonical_name);
        for (email, count) in &group.emails {
            println!("    {:<40} {} commits", email, count);
        }
        if group.emails.len() >= 2 {
            let canonical = &group.emails[0].0;
            let suggestions: Vec<String> = group.emails[1..]
                .iter()
                .map(|(alias, _)| {
                    format_mailmap_entry(&group.canonical_name, canonical, alias)
                })
                .collect();
            println!("\n  Suggested .mailmap entries:");
            for s in &suggestions {
                println!("    {}", s);
                entries_to_write.push(s.clone());
            }
        }
        println!();
    }

    if args.write {
        write_mailmap_entries(&repo_path, &entries_to_write)?;
        println!("Written to .mailmap.");
    } else {
        println!("Run with --write to append to .mailmap");
    }

    Ok(())
}

fn write_mailmap_entries(repo_path: &std::path::Path, entries: &[String]) -> Result<()> {
    let path = repo_path.join(".mailmap");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut to_add: Vec<&str> = entries
        .iter()
        .filter(|e| !existing.contains(e.as_str()))
        .map(|s| s.as_str())
        .collect();

    if to_add.is_empty() {
        println!("All suggested entries already present in .mailmap.");
        return Ok(());
    }

    let mut content = existing;
    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    for entry in &mut to_add {
        content.push_str(entry);
        content.push('\n');
    }
    std::fs::write(&path, content)?;
    Ok(())
}
```

**Step 4: Run tests**

```bash
rtk cargo test contributors
```

Expected: all pass

**Step 5: Run full test suite**

```bash
rtk cargo test
```

Expected: all pass, no warnings (`RUSTFLAGS=-D warnings rtk cargo test`)

**Step 6: Smoke-test the command**

```bash
rtk cargo run -- contributors .
```

Expected: either "No suspected duplicates found." or a list of groups for this repo.

**Step 7: Commit**

```bash
rtk git add src/contributors.rs src/main.rs
rtk git commit -m "feat: add contributors subcommand to detect duplicate identities"
```

---

## Verification

```bash
# Full test suite with warnings-as-errors
RUSTFLAGS=-D warnings rtk cargo test

# CLI smoke-test
rtk cargo run -- contributors --help
rtk cargo run -- contributors .
rtk cargo run -- contributors . --write   # only if duplicates found

# Regression: existing analyze still works
rtk cargo run -- analyze .
```
