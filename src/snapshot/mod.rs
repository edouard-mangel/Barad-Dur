use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub type AuthorId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitId(pub u32);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommitInterner {
    strings: Vec<String>,
}

impl CommitInterner {
    pub fn intern(&mut self, sha: &str) -> CommitId {
        if let Some(pos) = self.strings.iter().position(|s| s == sha) {
            CommitId(pos as u32)
        } else {
            let id = CommitId(self.strings.len() as u32);
            self.strings.push(sha.to_string());
            id
        }
    }

    pub fn resolve(&self, id: CommitId) -> &str {
        &self.strings[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub additions: u32,
    pub deletions: u32,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: CommitId,
    pub author: AuthorId,
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub files_changed: Vec<FileChange>,
    pub is_merge: bool,
    pub parent_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub is_binary: bool,
    pub depth: usize,
    /// Git blob OID (40-char hex). Content-addressed — same content = same OID.
    pub blob_oid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub id: AuthorId,
    pub name: String,
    pub email: String,
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub author_id: AuthorId,
    pub timestamp: DateTime<Utc>,
    /// Number of consecutive lines this entry represents (run-length encoding).
    pub line_count: usize,
}

impl BlameLine {
    pub fn new(author_id: AuthorId, timestamp: DateTime<Utc>) -> Self {
        Self {
            author_id,
            timestamp,
            line_count: 1,
        }
    }
}

/// Compress a sequence of blame lines by merging consecutive runs with the same author.
/// Reduces memory: 500 lines by one author become 1 entry with `line_count = 500`.
pub fn compress_blame(lines: Vec<BlameLine>) -> Vec<BlameLine> {
    if lines.is_empty() {
        return lines;
    }
    let mut compressed = Vec::with_capacity(lines.len() / 4);
    let mut current = lines[0].clone();
    for line in lines.into_iter().skip(1) {
        if line.author_id == current.author_id {
            current.line_count += line.line_count;
        } else {
            compressed.push(current);
            current = line;
        }
    }
    compressed.push(current);
    compressed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMetrics {
    pub name: String,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub max_nesting_depth: u32,
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileComplexity {
    pub total_lines: usize,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    pub functions: Vec<FunctionMetrics>,
    pub max_nesting_depth: u32,
    pub nesting_variance: f64,
}

/// Pressman coupling taxonomy rung, ordered worst → least severe.
/// Only the statically detectable rungs are represented (data/stamp omitted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CouplingKind {
    /// Reaching into another module's internals (e.g. `#[path]` imports).
    Content,
    /// Shared mutable global state (e.g. `static mut`, exported `let`).
    Common,
    /// Flag parameter steering a public function's internal control flow.
    Control,
}

/// A single evidenced coupling finding produced by the collector's AST pass
/// (or, for barrel bypass, derived from the import graph at metric time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CouplingFinding {
    pub path: PathBuf,
    /// 1-based line. `None` for graph-derived findings (no line information).
    pub line: Option<usize>,
    pub kind: CouplingKind,
    /// Short human-readable snippet, e.g. `static mut CACHE: usize = 0;`.
    pub evidence: String,
}

/// A `class … extends …` site in a TS/JS file, with its base resolved as
/// far as static analysis allows. Produced by the collector; inheritance
/// depth (DIT) is computed at metric time so the threshold knob stays live
/// without re-collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassRecord {
    pub path: PathBuf,
    /// 1-based declaration line.
    pub line: usize,
    pub class_name: String,
    pub base: BaseRef,
}

/// The extends-target of a class record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BaseRef {
    /// Base class assumed declared in the same file.
    SameFile(String),
    /// Base imported from a resolved project-local file.
    Resolved { path: PathBuf, name: String },
    /// External package, non-identifier extends expression, or unresolved
    /// specifier — terminates depth counting.
    Unresolvable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub default_months: u32,
}

impl Default for TimeWindow {
    fn default() -> Self {
        let now = Utc::now();
        TimeWindow {
            since: Some(now - Duration::days(180)),
            until: Some(now),
            default_months: 6,
        }
    }
}

impl TimeWindow {
    pub fn full_history() -> Self {
        TimeWindow {
            since: None,
            until: None,
            default_months: 0,
        }
    }

    pub fn contains(&self, timestamp: &DateTime<Utc>) -> bool {
        if let Some(since) = &self.since {
            if timestamp < since {
                return false;
            }
        }
        if let Some(until) = &self.until {
            if timestamp > until {
                return false;
            }
        }
        true
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub path: PathBuf,
    pub name: String,
    pub default_branch: String,
    pub time_window: TimeWindow,
    pub head_commit: String,
    pub created_at: DateTime<Utc>,

    pub commits: Vec<Commit>,
    pub files: Vec<FileEntry>,
    pub authors: Vec<Author>,
    pub blame_map: HashMap<PathBuf, Vec<BlameLine>>,

    pub commits_by_author: HashMap<AuthorId, Vec<CommitId>>,
    pub commits_by_file: HashMap<PathBuf, Vec<CommitId>>,
    pub file_change_pairs: Vec<(PathBuf, PathBuf, usize)>,
    pub file_metrics: HashMap<PathBuf, FileComplexity>,
    pub import_graph: HashMap<PathBuf, Vec<PathBuf>>,
    pub coupling_findings: Vec<CouplingFinding>,
    pub class_records: Vec<ClassRecord>,
    pub commit_interner: CommitInterner,
}

impl RepoSnapshot {
    pub fn new(path: PathBuf, name: String, branch: String, window: TimeWindow) -> Self {
        RepoSnapshot {
            path,
            name,
            default_branch: branch,
            time_window: window,
            head_commit: String::new(),
            created_at: Utc::now(),
            commits: Vec::new(),
            files: Vec::new(),
            authors: Vec::new(),
            blame_map: HashMap::new(),
            commits_by_author: HashMap::new(),
            commits_by_file: HashMap::new(),
            file_change_pairs: Vec::new(),
            file_metrics: HashMap::new(),
            import_graph: HashMap::new(),
            coupling_findings: Vec::new(),
            class_records: Vec::new(),
            commit_interner: CommitInterner::default(),
        }
    }

    /// Resolve a `CommitId` back to its SHA string.
    pub fn resolve_commit(&self, id: CommitId) -> &str {
        self.commit_interner.resolve(id)
    }

    /// Build all derived indexes from the core data.
    pub fn build_indexes(&mut self) {
        self.build_commits_by_author();
        self.build_commits_by_file();
        self.build_file_change_pairs();
    }

    fn build_commits_by_author(&mut self) {
        self.commits_by_author.clear();
        for commit in &self.commits {
            self.commits_by_author
                .entry(commit.author)
                .or_default()
                .push(commit.id);
        }
    }

    fn build_commits_by_file(&mut self) {
        self.commits_by_file.clear();
        for commit in &self.commits {
            for fc in &commit.files_changed {
                self.commits_by_file
                    .entry(fc.path.clone())
                    .or_default()
                    .push(commit.id);
            }
        }
    }

    fn build_file_change_pairs(&mut self) {
        use std::collections::HashSet;

        // Only consider files present in the (already filtered) file tree.
        // This ensures excluded paths (translations, config, lockfiles) don't
        // appear in coupling pairs.
        let known_files: HashSet<&PathBuf> = self.files.iter().map(|f| &f.path).collect();

        let mut pairs = count_co_changed_pairs(&self.commits, &known_files);

        // Sort by count descending for easy access
        pairs.sort_by_key(|p| std::cmp::Reverse(p.2));
        self.file_change_pairs = pairs;
    }
}

/// Count co-changed file pairs across commits, filtering to known files.
/// Returns pairs with at least 3 co-changes, unsorted.
fn count_co_changed_pairs(
    commits: &[Commit],
    known_files: &std::collections::HashSet<&PathBuf>,
) -> Vec<(PathBuf, PathBuf, usize)> {
    use std::collections::HashMap as Map;

    let mut pair_counts: Map<(PathBuf, PathBuf), usize> = Map::new();

    for commit in commits {
        let paths: Vec<&PathBuf> = commit
            .files_changed
            .iter()
            .map(|fc| &fc.path)
            .filter(|p| known_files.contains(p))
            .collect();
        for i in 0..paths.len() {
            for j in (i + 1)..paths.len() {
                let (a, b) = if paths[i] < paths[j] {
                    (paths[i].clone(), paths[j].clone())
                } else {
                    (paths[j].clone(), paths[i].clone())
                };
                *pair_counts.entry((a, b)).or_insert(0) += 1;
            }
        }
    }

    pair_counts
        .into_iter()
        .filter(|&(_, count)| count >= 3)
        .map(|((a, b), count)| (a, b, count))
        .collect()
}

#[cfg(test)]
mod tests;
