# Code-Maat File Analysis Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add per-file analysis (hotspots, coupling, author ownership, code age) with heuristic static metrics (LOC, cyclomatic complexity, public methods, properties) to the JSON report and a new tabbed dashboard view.

**Architecture:** Heuristic static analysis runs during collection over working-tree files, stored in `RepoSnapshot::file_metrics`. The scorer assembles four new top-level arrays in `AnalysisReport` (`file_hotspots`, `coupling_pairs`, `author_ownership`, `file_ages`). The dashboard Report page gains a tab bar with four new views consuming these arrays.

**Tech Stack:** Rust (regex crate already present via dependencies, `std::fs::read_to_string`), React 19 + D3 7 + Tailwind 4 (existing dashboard stack).

---

## Task 1: Add `FileComplexity` to `RepoSnapshot`

**Files:**
- Modify: `src/snapshot.rs`

**Step 1: Add the struct and field**

In `src/snapshot.rs`, add after the `BlameLine` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileComplexity {
    pub total_lines: usize,
    pub loc: usize,              // non-empty, non-comment lines
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
}
```

Then add to `RepoSnapshot`:

```rust
pub file_metrics: HashMap<PathBuf, FileComplexity>,
```

Initialize it in `RepoSnapshot::new`:

```rust
file_metrics: HashMap::new(),
```

**Step 2: Write a test**

In `src/snapshot.rs` tests block:

```rust
#[test]
fn file_metrics_starts_empty() {
    let snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp/test"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    assert!(snapshot.file_metrics.is_empty());
}
```

**Step 3: Run test**

```bash
cargo test snapshot::tests::file_metrics_starts_empty
```
Expected: PASS

**Step 4: Verify serialization still works**

```bash
cargo test snapshot::tests::repo_snapshot_serialization_roundtrip
```
Expected: PASS

**Step 5: Commit**

```bash
git add src/snapshot.rs
git commit -m "feat: add FileComplexity to RepoSnapshot"
```

---

## Task 2: Create heuristic complexity analyser

**Files:**
- Create: `src/metrics/complexity.rs`
- Modify: `src/metrics/mod.rs` (add `pub mod complexity;`)

**Step 1: Write the failing tests first**

Create `src/metrics/complexity.rs` with just the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_language() {
        assert!(matches!(detect_language("src/main.rs"), Language::Rust));
        assert!(matches!(detect_language("lib.rs"), Language::Rust));
    }

    #[test]
    fn detects_js_ts() {
        assert!(matches!(detect_language("app.ts"), Language::JsTs));
        assert!(matches!(detect_language("index.jsx"), Language::JsTs));
    }

    #[test]
    fn detects_python() {
        assert!(matches!(detect_language("script.py"), Language::Python));
    }

    #[test]
    fn detects_go() {
        assert!(matches!(detect_language("main.go"), Language::Go));
    }

    #[test]
    fn detects_jvm() {
        assert!(matches!(detect_language("Foo.java"), Language::Jvm));
        assert!(matches!(detect_language("Bar.kt"), Language::Jvm));
        assert!(matches!(detect_language("Baz.cs"), Language::Jvm));
    }

    #[test]
    fn loc_skips_blank_and_comment_lines() {
        let content = "// comment\n\nfn main() {}\n    // indented comment\nlet x = 1;\n";
        let result = analyse_content(content, Language::Rust);
        assert_eq!(result.total_lines, 5);
        assert_eq!(result.loc, 2); // fn main(){} and let x = 1;
    }

    #[test]
    fn cyclomatic_complexity_counts_decision_points() {
        let content = "if x { } else if y { } for i in v { } while z { } match a { _ => {} }";
        let result = analyse_content(content, Language::Rust);
        // if, else if, for, while, match = 5
        assert!(result.cyclomatic_complexity >= 5);
    }

    #[test]
    fn public_methods_rust() {
        let content = "pub fn foo() {}\nfn bar() {}\npub fn baz() {}\n";
        let result = analyse_content(content, Language::Rust);
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn public_methods_typescript() {
        let content = "export function foo() {}\nfunction bar() {}\nexport const baz = () => {}\n";
        let result = analyse_content(content, Language::JsTs);
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn properties_rust_struct_fields() {
        let content = "struct Foo {\n    pub x: i32,\n    pub y: String,\n    z: bool,\n}\n";
        let result = analyse_content(content, Language::Rust);
        assert_eq!(result.properties, 2);
    }
}
```

**Step 2: Run to verify all fail**

```bash
cargo test metrics::complexity
```
Expected: compile error (functions not defined yet)

**Step 3: Implement the module**

```rust
use std::path::Path;
use crate::snapshot::FileComplexity;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    Rust,
    JsTs,
    Python,
    Go,
    Jvm,   // Java, Kotlin, C#
    Generic,
}

pub fn detect_language(path: &str) -> Language {
    match Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("") {
        "rs" => Language::Rust,
        "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs" => Language::JsTs,
        "py" => Language::Python,
        "go" => Language::Go,
        "java" | "kt" | "kts" | "cs" => Language::Jvm,
        _ => Language::Generic,
    }
}

pub fn analyse_file(path: &Path, content: &str) -> FileComplexity {
    let lang = detect_language(&path.to_string_lossy());
    analyse_content(content, lang)
}

pub fn analyse_content(content: &str, lang: Language) -> FileComplexity {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let loc = lines.iter().filter(|l| {
        let t = l.trim();
        !t.is_empty() && !is_comment_line(t, lang)
    }).count();

    let cyclomatic_complexity = count_complexity(content);
    let public_methods = count_public_methods(content, lang);
    let properties = count_properties(content, lang);

    FileComplexity { total_lines, loc, cyclomatic_complexity, public_methods, properties }
}

fn is_comment_line(trimmed: &str, lang: Language) -> bool {
    match lang {
        Language::Rust | Language::JsTs | Language::Go | Language::Jvm => {
            trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
        }
        Language::Python => trimmed.starts_with('#'),
        Language::Generic => trimmed.starts_with("//") || trimmed.starts_with('#'),
    }
}

fn count_complexity(content: &str) -> u32 {
    // Count decision-point keywords across the whole file
    let keywords = [
        " if ", "\tif ", "(if ", "else if", "elif ",
        " for ", "\tfor ", " while ", "\twhile ",
        " match ", " switch ", " case ", " catch ", " except ",
        " loop ", "&&", "||", " ?? ",
    ];
    let mut count = 0u32;
    for kw in &keywords {
        count += content.matches(kw).count() as u32;
    }
    count
}

fn count_public_methods(content: &str, lang: Language) -> u32 {
    let mut count = 0u32;
    match lang {
        Language::Rust => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("pub fn ") || t.starts_with("pub async fn ") {
                    count += 1;
                }
            }
        }
        Language::JsTs => {
            for line in content.lines() {
                let t = line.trim();
                if (t.starts_with("export function ")
                    || t.starts_with("export async function ")
                    || t.starts_with("export const ")
                    || t.contains("public ") && t.contains('('))
                    && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::Python => {
            // Count top-level `def` and class-level `def` (all def is public by default)
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("def ") && !t.starts_with("def _") {
                    count += 1;
                }
            }
        }
        Language::Go => {
            // Exported Go functions start with uppercase after `func `
            for line in content.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("func ") {
                    // Skip method receivers: func (r Recv) Name(
                    let name_start = if rest.starts_with('(') {
                        rest.find(')').map(|i| &rest[i+1..]).unwrap_or("").trim()
                    } else {
                        rest
                    };
                    if name_start.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        count += 1;
                    }
                }
            }
        }
        Language::Jvm => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("public ") && t.contains('(') && !t.starts_with("public class")
                    && !t.starts_with("public interface") && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::Generic => {}
    }
    count
}

fn count_properties(content: &str, lang: Language) -> u32 {
    let mut count = 0u32;
    match lang {
        Language::Rust => {
            // pub field: Type inside struct blocks
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("pub ") && t.contains(':') && !t.starts_with("pub fn")
                    && !t.starts_with("pub struct") && !t.starts_with("pub enum")
                    && !t.starts_with("pub mod") && !t.starts_with("pub use")
                    && !t.starts_with("pub type") && !t.starts_with("pub trait")
                    && !t.starts_with("pub const") && !t.starts_with("pub static")
                    && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::JsTs => {
            for line in content.lines() {
                let t = line.trim();
                // this.x = ... or private/public x: type in class bodies
                if (t.starts_with("this.") && t.contains(" = "))
                    || ((t.starts_with("private ") || t.starts_with("public ")
                        || t.starts_with("protected ") || t.starts_with("readonly "))
                        && t.contains(':') && !t.contains('('))
                {
                    count += 1;
                }
            }
        }
        Language::Python => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("self.") && t.contains(" = ") {
                    count += 1;
                }
            }
        }
        Language::Go => {
            // Exported struct fields: lines inside struct with uppercase first char
            for line in content.lines() {
                let t = line.trim();
                if t.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && (t.contains("string") || t.contains("int") || t.contains("bool")
                        || t.contains("float") || t.contains("[]") || t.contains("map["))
                    && !t.contains('(') && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::Jvm => {
            for line in content.lines() {
                let t = line.trim();
                if (t.starts_with("private ") || t.starts_with("public ")
                    || t.starts_with("protected "))
                    && !t.contains('(') && t.contains(';') && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::Generic => {}
    }
    count
}
```

Add to `src/metrics/mod.rs`:
```rust
pub mod complexity;
```

**Step 4: Run the tests**

```bash
cargo test metrics::complexity
```
Expected: All PASS

**Step 5: Commit**

```bash
git add src/metrics/complexity.rs src/metrics/mod.rs
git commit -m "feat: add heuristic complexity analyser"
```

---

## Task 3: Collect file content during snapshot build

**Files:**
- Modify: `src/collector/mod.rs`
- Modify: `src/snapshot.rs` (already has `file_metrics` field from Task 1)

**Step 1: Add `collect_file_metrics` to Collector**

In `src/collector/mod.rs`, add after `collect_blame`:

```rust
/// Analyse working-tree files for static complexity metrics.
pub fn collect_file_metrics(
    &self,
    files: &[crate::snapshot::FileEntry],
) -> HashMap<PathBuf, crate::snapshot::FileComplexity> {
    use crate::metrics::complexity;

    let root = self.repo_path();
    let mut map = HashMap::new();

    for entry in files {
        if entry.is_binary {
            continue;
        }
        let abs_path = root.join(&entry.path);
        if let Ok(content) = std::fs::read_to_string(&abs_path) {
            let metrics = complexity::analyse_file(&entry.path, &content);
            map.insert(entry.path.clone(), metrics);
        }
    }
    map
}
```

**Step 2: Call it in `collect_snapshot_with_progress`**

In the same file, after `let blame_map = self.collect_blame(...)`:

```rust
if let Some(sp) = &spinner {
    sp.set_message("Analysing file complexity...");
}
let file_metrics = self.collect_file_metrics(&files);
```

Then add `file_metrics` to the `RepoSnapshot { ... }` struct literal:

```rust
file_metrics,
```

**Step 3: Write an integration smoke test**

In `src/collector/mod.rs` tests (or inline):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::TimeWindow;

    #[test]
    fn collect_file_metrics_does_not_panic_on_real_repo() {
        // Uses the barad-dur repo itself as test fixture
        let collector = Collector::open(
            std::path::Path::new("."),
            TimeWindow::default(),
        ).expect("should open repo");
        let files = collector.collect_files().expect("should collect files");
        let metrics = collector.collect_file_metrics(&files);
        // Should have at least some rust files
        assert!(!metrics.is_empty());
        let rs_file = metrics.keys().find(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("rs")
        });
        assert!(rs_file.is_some(), "expected at least one .rs file");
    }
}
```

**Step 4: Run the test**

```bash
cargo test collector::tests::collect_file_metrics_does_not_panic_on_real_repo
```
Expected: PASS

**Step 5: Commit**

```bash
git add src/collector/mod.rs
git commit -m "feat: collect per-file complexity metrics during snapshot build"
```

---

## Task 4: Add file analysis types and computation to scorer

**Files:**
- Modify: `src/scorer.rs`

**Step 1: Add new types**

In `src/scorer.rs`, add after `RemoteMeta`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct HotspotFile {
    pub path: String,
    pub churn_count: usize,
    pub loc: usize,
    pub total_lines: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    pub hotspot_score: f64,   // 0.0–100.0
}

#[derive(Debug, Clone, Serialize)]
pub struct CouplingPair {
    pub file_a: String,
    pub file_b: String,
    pub co_changes: usize,
    pub coupling_pct: f64,   // co_changes / min(changes_a, changes_b) * 100
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorShare {
    pub name: String,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOwnership {
    pub path: String,
    pub authors: Vec<AuthorShare>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileAge {
    pub path: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub days_since_modified: i64,
}
```

**Step 2: Add fields to `AnalysisReport`**

```rust
pub file_hotspots: Vec<HotspotFile>,
pub coupling_pairs: Vec<CouplingPair>,
pub author_ownership: Vec<FileOwnership>,
pub file_ages: Vec<FileAge>,
```

**Step 3: Add computation functions**

Add these private functions in `src/scorer.rs`:

```rust
fn build_hotspots(snapshot: &RepoSnapshot) -> Vec<HotspotFile> {
    use std::path::PathBuf;

    // Collect raw data per file
    let mut files: Vec<HotspotFile> = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary)
        .map(|f| {
            let churn = snapshot.commits_by_file
                .get(&f.path).map(|v| v.len()).unwrap_or(0);
            let metrics = snapshot.file_metrics.get(&f.path).cloned()
                .unwrap_or_default();
            HotspotFile {
                path: f.path.to_string_lossy().to_string(),
                churn_count: churn,
                loc: metrics.loc,
                total_lines: metrics.total_lines,
                cyclomatic_complexity: metrics.cyclomatic_complexity,
                public_methods: metrics.public_methods,
                properties: metrics.properties,
                hotspot_score: 0.0,
            }
        })
        .collect();

    if files.is_empty() {
        return files;
    }

    // Normalize each dimension to 0-100, then combine
    let max_churn = files.iter().map(|f| f.churn_count).max().unwrap_or(1).max(1);
    let max_cc   = files.iter().map(|f| f.cyclomatic_complexity as usize).max().unwrap_or(1).max(1);
    let max_loc  = files.iter().map(|f| f.loc).max().unwrap_or(1).max(1);

    for f in &mut files {
        let churn_norm = f.churn_count as f64 / max_churn as f64;
        let cc_norm    = f.cyclomatic_complexity as f64 / max_cc as f64;
        let loc_norm   = f.loc as f64 / max_loc as f64;
        f.hotspot_score = (churn_norm * 0.5 + cc_norm * 0.3 + loc_norm * 0.2) * 100.0;
    }

    files.sort_by(|a, b| b.hotspot_score.partial_cmp(&a.hotspot_score).unwrap());
    files
}

fn build_coupling_pairs(snapshot: &RepoSnapshot) -> Vec<CouplingPair> {
    snapshot.file_change_pairs.iter().map(|(a, b, co)| {
        let a_changes = snapshot.commits_by_file.get(a).map(|v| v.len()).unwrap_or(0);
        let b_changes = snapshot.commits_by_file.get(b).map(|v| v.len()).unwrap_or(0);
        let min_changes = a_changes.min(b_changes).max(1);
        let coupling_pct = (*co as f64 / min_changes as f64 * 100.0).min(100.0);
        CouplingPair {
            file_a: a.to_string_lossy().to_string(),
            file_b: b.to_string_lossy().to_string(),
            co_changes: *co,
            coupling_pct,
        }
    }).collect()
}

fn build_author_ownership(snapshot: &RepoSnapshot) -> Vec<FileOwnership> {
    snapshot.blame_map.iter().map(|(path, lines)| {
        let mut author_counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for line in lines {
            *author_counts.entry(line.author_id).or_insert(0) += 1;
        }
        let total = lines.len().max(1);
        let mut authors: Vec<AuthorShare> = author_counts.into_iter().map(|(id, count)| {
            let name = snapshot.authors.get(id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| format!("author-{}", id));
            AuthorShare { name, pct: count as f64 / total as f64 * 100.0 }
        }).collect();
        authors.sort_by(|a, b| b.pct.partial_cmp(&a.pct).unwrap());
        FileOwnership { path: path.to_string_lossy().to_string(), authors }
    }).collect()
}

fn build_file_ages(snapshot: &RepoSnapshot) -> Vec<FileAge> {
    let now = chrono::Utc::now();
    let mut ages: Vec<FileAge> = snapshot.files.iter().filter(|f| !f.is_binary).map(|f| {
        // Find the most recent commit that touched this file
        let last_modified = snapshot.commits_by_file
            .get(&f.path)
            .and_then(|commit_ids| {
                commit_ids.iter()
                    .filter_map(|cid| snapshot.commits.iter().find(|c| &c.id == cid))
                    .map(|c| c.timestamp)
                    .max()
            })
            .unwrap_or(snapshot.created_at - chrono::Duration::days(365 * 5));

        let days = (now - last_modified).num_days().max(0);
        FileAge {
            path: f.path.to_string_lossy().to_string(),
            last_modified,
            days_since_modified: days,
        }
    }).collect();

    ages.sort_by(|a, b| b.days_since_modified.cmp(&a.days_since_modified));
    ages
}
```

**Step 4: Wire into `build_report`**

Update `build_report` to call these:

```rust
pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
) -> AnalysisReport {
    let overall_score = compute_overall_score(&categories);
    let top_actions = generate_top_actions(&categories);

    let file_hotspots   = build_hotspots(snapshot);
    let coupling_pairs  = build_coupling_pairs(snapshot);
    let author_ownership = build_author_ownership(snapshot);
    let file_ages       = build_file_ages(snapshot);

    AnalysisReport {
        // ... existing fields ...
        file_hotspots,
        coupling_pairs,
        author_ownership,
        file_ages,
    }
}
```

**Step 5: Write tests**

In `src/scorer.rs` tests:

```rust
#[test]
fn build_hotspots_ranks_by_score() {
    use crate::snapshot::*;
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"), "t".into(), "main".into(), TimeWindow::default(),
    );
    // hot.rs: 10 commits, cold.rs: 1 commit
    snapshot.files = vec![
        FileEntry { path: "hot.rs".into(), size_bytes: 5000, is_binary: false, depth: 1 },
        FileEntry { path: "cold.rs".into(), size_bytes: 100, is_binary: false, depth: 1 },
    ];
    snapshot.commits_by_file.insert("hot.rs".into(), (0..10).map(|i| format!("c{}", i)).collect());
    snapshot.commits_by_file.insert("cold.rs".into(), vec!["c0".into()]);
    let hotspots = build_hotspots(&snapshot);
    assert_eq!(hotspots[0].path, "hot.rs");
    assert!(hotspots[0].hotspot_score > hotspots[1].hotspot_score);
}

#[test]
fn build_coupling_pairs_computes_pct() {
    use crate::snapshot::*;
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"), "t".into(), "main".into(), TimeWindow::default(),
    );
    snapshot.file_change_pairs = vec![("a.rs".into(), "b.rs".into(), 8)];
    snapshot.commits_by_file.insert("a.rs".into(), (0..10).map(|i| format!("c{}", i)).collect());
    snapshot.commits_by_file.insert("b.rs".into(), (0..10).map(|i| format!("c{}", i)).collect());
    let pairs = build_coupling_pairs(&snapshot);
    assert_eq!(pairs.len(), 1);
    assert!((pairs[0].coupling_pct - 80.0).abs() < 1.0);
}

#[test]
fn build_file_ages_sorts_oldest_first() {
    use crate::snapshot::*;
    use chrono::{Duration, Utc};
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"), "t".into(), "main".into(), TimeWindow::default(),
    );
    let now = Utc::now();
    snapshot.files = vec![
        FileEntry { path: "new.rs".into(), size_bytes: 100, is_binary: false, depth: 1 },
        FileEntry { path: "old.rs".into(), size_bytes: 100, is_binary: false, depth: 1 },
    ];
    snapshot.commits = vec![
        Commit { id: "c1".into(), author: 0, timestamp: now - Duration::days(5), message: "".into(), files_changed: vec![], is_merge: false, parent_count: 1 },
        Commit { id: "c2".into(), author: 0, timestamp: now - Duration::days(100), message: "".into(), files_changed: vec![], is_merge: false, parent_count: 1 },
    ];
    snapshot.commits_by_file.insert("new.rs".into(), vec!["c1".into()]);
    snapshot.commits_by_file.insert("old.rs".into(), vec!["c2".into()]);
    let ages = build_file_ages(&snapshot);
    assert_eq!(ages[0].path, "old.rs");
    assert!(ages[0].days_since_modified > ages[1].days_since_modified);
}
```

**Step 6: Run all scorer tests**

```bash
cargo test scorer::
```
Expected: All PASS

**Step 7: Commit**

```bash
git add src/scorer.rs
git commit -m "feat: add file hotspot, coupling, ownership, age computation to scorer"
```

---

## Task 5: Verify full build and sample JSON

**Step 1: Full build**

```bash
cargo build --release 2>&1 | grep -E "error|warning: unused"
```
Expected: No errors

**Step 2: Regenerate sample JSON and check new fields**

```bash
cargo run --release -- analyze . --json > dashboard/sample.json
cat dashboard/sample.json | python3 -c "import json,sys; r=json.load(sys.stdin); print('hotspots:', len(r['file_hotspots'])); print('coupling:', len(r['coupling_pairs'])); print('ownership:', len(r['author_ownership'])); print('ages:', len(r['file_ages']))"
```
Expected: all counts > 0

**Step 3: Run full test suite**

```bash
cargo test 2>&1 | tail -10
```
Expected: all tests pass

**Step 4: Commit**

```bash
git add dashboard/sample.json
git commit -m "chore: regenerate sample.json with file analysis fields"
```

---

## Task 6: Update dashboard TypeScript types

**Files:**
- Modify: `dashboard/src/types.ts`

**Step 1: Add new interfaces**

In `dashboard/src/types.ts`, add after the existing interfaces:

```typescript
export interface HotspotFile {
  path: string
  churn_count: number
  loc: number
  total_lines: number
  cyclomatic_complexity: number
  public_methods: number
  properties: number
  hotspot_score: number
}

export interface CouplingPair {
  file_a: string
  file_b: string
  co_changes: number
  coupling_pct: number
}

export interface AuthorShare {
  name: string
  pct: number
}

export interface FileOwnership {
  path: string
  authors: AuthorShare[]
}

export interface FileAge {
  path: string
  last_modified: string   // ISO date string
  days_since_modified: number
}
```

**Step 2: Add fields to `AnalysisReport`**

```typescript
file_hotspots: HotspotFile[]
coupling_pairs: CouplingPair[]
author_ownership: FileOwnership[]
file_ages: FileAge[]
```

**Step 3: Update `isAnalysisReport` guard**

Add to the check:
```typescript
Array.isArray(r['file_hotspots']) &&
Array.isArray(r['coupling_pairs'])
```

**Step 4: Verify build**

```bash
cd dashboard && pnpm run build 2>&1 | tail -5
```
Expected: `✓ built in ...`

**Step 5: Commit**

```bash
git add dashboard/src/types.ts
git commit -m "feat(dashboard): add file analysis TypeScript types"
```

---

## Task 7: Add tab navigation to Report page

**Files:**
- Modify: `dashboard/src/pages/Report.tsx`

**Step 1: Add tab state and tab bar**

In `Report.tsx`, add tab state:

```typescript
type Tab = 'overview' | 'hotspots' | 'coupling' | 'ownership' | 'age'
const [activeTab, setActiveTab] = useState<Tab>('overview')
```

Add tab bar between the header divider and main content:

```tsx
{/* Tab bar */}
<div style={{ display: 'flex', gap: '0.25rem', marginBottom: '1.5rem', borderBottom: '1px solid rgba(255,255,255,0.06)', paddingBottom: '0' }}>
  {([
    ['overview', 'Overview'],
    ['hotspots', 'Hotspots'],
    ['coupling', 'Coupling'],
    ['ownership', 'Ownership'],
    ['age', 'Age'],
  ] as [Tab, string][]).map(([tab, label]) => (
    <button
      key={tab}
      onClick={() => setActiveTab(tab)}
      style={{
        background: 'none',
        border: 'none',
        borderBottom: activeTab === tab ? '2px solid #f59e0b' : '2px solid transparent',
        padding: '0.5rem 1rem',
        cursor: 'pointer',
        fontFamily: 'Syne, sans-serif',
        fontWeight: activeTab === tab ? 700 : 400,
        fontSize: '0.82rem',
        color: activeTab === tab ? '#f59e0b' : 'rgba(148, 163, 184, 0.6)',
        transition: 'all 0.15s ease',
        marginBottom: '-1px',
        letterSpacing: '0.04em',
      }}
    >
      {label}
    </button>
  ))}
</div>
```

Wrap existing main grid + top actions in `{activeTab === 'overview' && (...)}`.

Add placeholders for other tabs (filled in Tasks 8–11):

```tsx
{activeTab === 'hotspots'  && <div style={{color:'rgba(148,163,184,0.4)',fontFamily:'JetBrains Mono',fontSize:'0.8rem'}}>Hotspots view coming…</div>}
{activeTab === 'coupling'  && <div style={{color:'rgba(148,163,184,0.4)',fontFamily:'JetBrains Mono',fontSize:'0.8rem'}}>Coupling view coming…</div>}
{activeTab === 'ownership' && <div style={{color:'rgba(148,163,184,0.4)',fontFamily:'JetBrains Mono',fontSize:'0.8rem'}}>Ownership view coming…</div>}
{activeTab === 'age'       && <div style={{color:'rgba(148,163,184,0.4)',fontFamily:'JetBrains Mono',fontSize:'0.8rem'}}>Age view coming…</div>}
```

**Step 2: Verify dev server**

```bash
cd dashboard && pnpm run build 2>&1 | tail -3
```
Expected: `✓ built`

**Step 3: Commit**

```bash
git add dashboard/src/pages/Report.tsx
git commit -m "feat(dashboard): add tab navigation to report page"
```

---

## Task 8: Build `HotspotsView` component

**Files:**
- Create: `dashboard/src/components/HotspotsView.tsx`
- Modify: `dashboard/src/pages/Report.tsx`

**Step 1: Create the component**

```tsx
import { useRef, useEffect, useState } from 'react'
import * as d3 from 'd3'
import type { HotspotFile } from '../types'

interface Props { files: HotspotFile[] }

type SortKey = 'hotspot_score' | 'churn_count' | 'cyclomatic_complexity' | 'loc'

export default function HotspotsView({ files }: Props) {
  const svgRef = useRef<SVGSVGElement>(null)
  const [sort, setSort] = useState<SortKey>('hotspot_score')
  const sorted = [...files].sort((a, b) => b[sort] - a[sort]).slice(0, 50)

  // D3 scatter: x=complexity, y=churn, radius=loc, color=hotspot_score
  useEffect(() => {
    if (!svgRef.current || files.length === 0) return
    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    const W = 560, H = 260, M = { top: 20, right: 20, bottom: 40, left: 50 }
    const w = W - M.left - M.right
    const h = H - M.top - M.bottom

    const g = svg.append('g').attr('transform', `translate(${M.left},${M.top})`)

    const xScale = d3.scaleLinear()
      .domain([0, d3.max(files, f => f.cyclomatic_complexity) ?? 1])
      .range([0, w])

    const yScale = d3.scaleLinear()
      .domain([0, d3.max(files, f => f.churn_count) ?? 1])
      .range([h, 0])

    const rScale = d3.scaleSqrt()
      .domain([0, d3.max(files, f => f.loc) ?? 1])
      .range([2, 14])

    // Axes
    g.append('g').attr('transform', `translate(0,${h})`)
      .call(d3.axisBottom(xScale).ticks(5))
      .call(ax => ax.select('.domain').attr('stroke', 'rgba(255,255,255,0.1)'))
      .call(ax => ax.selectAll('text').attr('fill', 'rgba(148,163,184,0.6)').attr('font-size', '9').attr('font-family', 'JetBrains Mono'))
      .call(ax => ax.selectAll('line').attr('stroke', 'rgba(255,255,255,0.1)'))

    g.append('g')
      .call(d3.axisLeft(yScale).ticks(4))
      .call(ax => ax.select('.domain').attr('stroke', 'rgba(255,255,255,0.1)'))
      .call(ax => ax.selectAll('text').attr('fill', 'rgba(148,163,184,0.6)').attr('font-size', '9').attr('font-family', 'JetBrains Mono'))
      .call(ax => ax.selectAll('line').attr('stroke', 'rgba(255,255,255,0.1)'))

    // Axis labels
    g.append('text').attr('x', w / 2).attr('y', h + 34)
      .attr('text-anchor', 'middle').attr('fill', 'rgba(148,163,184,0.4)').attr('font-size', '9').attr('font-family', 'Syne').text('Cyclomatic complexity →')
    g.append('text').attr('transform', 'rotate(-90)').attr('x', -h / 2).attr('y', -38)
      .attr('text-anchor', 'middle').attr('fill', 'rgba(148,163,184,0.4)').attr('font-size', '9').attr('font-family', 'Syne').text('Churn count →')

    // Bubbles
    const colorScale = d3.scaleLinear<string>()
      .domain([0, 50, 100])
      .range(['#10b981', '#f59e0b', '#ef4444'])

    g.selectAll('circle')
      .data(files)
      .join('circle')
      .attr('cx', f => xScale(f.cyclomatic_complexity))
      .attr('cy', f => yScale(f.churn_count))
      .attr('r', f => rScale(f.loc))
      .attr('fill', f => colorScale(f.hotspot_score))
      .attr('fill-opacity', 0.55)
      .attr('stroke', f => colorScale(f.hotspot_score))
      .attr('stroke-width', 1)
      .append('title')
      .text(f => `${f.path}\nscore: ${f.hotspot_score.toFixed(0)}\nchurn: ${f.churn_count}\ncc: ${f.cyclomatic_complexity}\nloc: ${f.loc}`)
  }, [files])

  const colStyle = (key: SortKey): React.CSSProperties => ({
    cursor: 'pointer',
    color: sort === key ? '#f59e0b' : 'rgba(148,163,184,0.5)',
    userSelect: 'none',
  })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
      {/* Scatter */}
      <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
        <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: '0.75rem' }}>
          Hotspot quadrant — bubble size = LOC
        </p>
        <svg ref={svgRef} width={560} height={260} />
      </div>

      {/* Table */}
      <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
          <thead>
            <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
              <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle('hotspot_score') }} onClick={() => setSort('hotspot_score')}>Score ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle('churn_count') }} onClick={() => setSort('churn_count')}>Churn ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle('cyclomatic_complexity') }} onClick={() => setSort('cyclomatic_complexity')}>CC ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle('loc') }} onClick={() => setSort('loc')}>LOC ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Methods</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Props</th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((f, i) => {
              const score = f.hotspot_score
              const color = score > 70 ? '#ef4444' : score > 40 ? '#f59e0b' : '#10b981'
              return (
                <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
                  <td style={{ padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.8)', maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
                    title={f.path}>
                    {f.path.split('/').pop()}
                    <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>
                      {f.path.includes('/') ? f.path.substring(0, f.path.lastIndexOf('/') + 1) : ''}
                    </span>
                  </td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color, fontWeight: 600 }}>{score.toFixed(0)}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.7)' }}>{f.churn_count}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.7)' }}>{f.cyclomatic_complexity}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.7)' }}>{f.loc}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.5)' }}>{f.public_methods}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.5)' }}>{f.properties}</td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}
```

**Step 2: Wire into Report.tsx**

Replace the hotspots placeholder with:
```tsx
{activeTab === 'hotspots' && <HotspotsView files={report.file_hotspots} />}
```

**Step 3: Build**

```bash
cd dashboard && pnpm run build 2>&1 | tail -3
```
Expected: `✓ built`

**Step 4: Commit**

```bash
git add dashboard/src/components/HotspotsView.tsx dashboard/src/pages/Report.tsx
git commit -m "feat(dashboard): add hotspot scatter plot and table"
```

---

## Task 9: Build `CouplingView` component

**Files:**
- Create: `dashboard/src/components/CouplingView.tsx`
- Modify: `dashboard/src/pages/Report.tsx`

**Step 1: Create the component**

```tsx
import type { CouplingPair } from '../types'

interface Props { pairs: CouplingPair[] }

export default function CouplingView({ pairs }: Props) {
  const sorted = [...pairs].sort((a, b) => b.coupling_pct - a.coupling_pct)

  if (sorted.length === 0) {
    return <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>No coupling pairs detected (threshold: 3 co-changes).</p>
  }

  return (
    <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
      <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: '0.75rem' }}>
        Temporal coupling — files that change together
      </p>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File A</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File B</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Co-changes</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400, minWidth: 160 }}>Coupling %</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((p, i) => {
            const pct = p.coupling_pct
            const color = pct > 70 ? '#ef4444' : pct > 40 ? '#f59e0b' : '#10b981'
            return (
              <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
                <td style={{ padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.8)', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={p.file_a}>
                  {p.file_a.split('/').pop()}
                </td>
                <td style={{ padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.8)', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={p.file_b}>
                  {p.file_b.split('/').pop()}
                </td>
                <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.6)' }}>{p.co_changes}</td>
                <td style={{ padding: '0.4rem 0.5rem' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                    <div style={{ flex: 1, height: 4, backgroundColor: 'rgba(255,255,255,0.06)', borderRadius: 2 }}>
                      <div style={{ width: `${pct}%`, height: '100%', backgroundColor: color, borderRadius: 2, boxShadow: `0 0 6px ${color}` }} />
                    </div>
                    <span style={{ color, fontWeight: 600, minWidth: '2.5rem', textAlign: 'right' }}>{pct.toFixed(0)}%</span>
                  </div>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
```

**Step 2: Wire into Report.tsx**

```tsx
{activeTab === 'coupling' && <CouplingView pairs={report.coupling_pairs} />}
```

**Step 3: Build and commit**

```bash
cd dashboard && pnpm run build 2>&1 | tail -3
git add dashboard/src/components/CouplingView.tsx dashboard/src/pages/Report.tsx
git commit -m "feat(dashboard): add coupling pairs table"
```

---

## Task 10: Build `OwnershipView` component

**Files:**
- Create: `dashboard/src/components/OwnershipView.tsx`
- Modify: `dashboard/src/pages/Report.tsx`

**Step 1: Create the component**

```tsx
import type { FileOwnership } from '../types'

interface Props { ownership: FileOwnership[] }

// Consistent color per author name (hash-based)
function authorColor(name: string, idx: number): string {
  const palette = ['#f59e0b', '#10b981', '#3b82f6', '#a78bfa', '#f472b6', '#34d399', '#fb923c', '#60a5fa']
  return palette[idx % palette.length]
}

export default function OwnershipView({ ownership }: Props) {
  // Only show files with at least 2 authors (interesting cases)
  const interesting = [...ownership]
    .filter(f => f.authors.length >= 1)
    .sort((a, b) => {
      // Sort by number of authors desc (most fragmented first)
      if (b.authors.length !== a.authors.length) return b.authors.length - a.authors.length
      // Then by lowest top-author pct (most evenly split)
      return (a.authors[0]?.pct ?? 100) - (b.authors[0]?.pct ?? 100)
    })
    .slice(0, 60)

  if (interesting.length === 0) {
    return <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>No ownership data available.</p>
  }

  // Build global author→color map
  const allAuthors = Array.from(new Set(ownership.flatMap(f => f.authors.map(a => a.name))))
  const authorColorMap = Object.fromEntries(allAuthors.map((name, i) => [name, authorColor(name, i)]))

  return (
    <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
      <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: '0.75rem' }}>
        Author ownership — blame distribution per file
      </p>

      {/* Legend */}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem', marginBottom: '1rem' }}>
        {allAuthors.slice(0, 8).map(name => (
          <div key={name} style={{ display: 'flex', alignItems: 'center', gap: '0.3rem' }}>
            <div style={{ width: 8, height: 8, borderRadius: '50%', backgroundColor: authorColorMap[name] }} />
            <span style={{ fontFamily: 'JetBrains Mono', fontSize: '0.65rem', color: 'rgba(148,163,184,0.6)' }}>{name}</span>
          </div>
        ))}
      </div>

      <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Ownership</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Authors</th>
          </tr>
        </thead>
        <tbody>
          {interesting.map((f, i) => (
            <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
              <td style={{ padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.8)', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={f.path}>
                {f.path.split('/').pop()}
                <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>
                  {f.path.includes('/') ? f.path.substring(0, f.path.lastIndexOf('/') + 1) : ''}
                </span>
              </td>
              <td style={{ padding: '0.4rem 0.5rem', minWidth: 200 }}>
                {/* Stacked bar */}
                <div style={{ display: 'flex', height: 8, borderRadius: 4, overflow: 'hidden', gap: '1px' }}>
                  {f.authors.map((a, j) => (
                    <div
                      key={j}
                      title={`${a.name}: ${a.pct.toFixed(0)}%`}
                      style={{
                        width: `${a.pct}%`,
                        backgroundColor: authorColorMap[a.name] ?? '#4a5568',
                        flexShrink: 0,
                      }}
                    />
                  ))}
                </div>
                <div style={{ marginTop: '0.2rem', fontSize: '0.62rem', color: 'rgba(148,163,184,0.4)' }}>
                  {f.authors[0] ? `${f.authors[0].name} ${f.authors[0].pct.toFixed(0)}%` : ''}
                </div>
              </td>
              <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: f.authors.length > 3 ? '#f59e0b' : 'rgba(148,163,184,0.5)' }}>
                {f.authors.length}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
```

**Step 2: Wire into Report.tsx**

```tsx
{activeTab === 'ownership' && <OwnershipView ownership={report.author_ownership} />}
```

**Step 3: Build and commit**

```bash
cd dashboard && pnpm run build 2>&1 | tail -3
git add dashboard/src/components/OwnershipView.tsx dashboard/src/pages/Report.tsx
git commit -m "feat(dashboard): add author ownership stacked bar table"
```

---

## Task 11: Build `AgeView` component

**Files:**
- Create: `dashboard/src/components/AgeView.tsx`
- Modify: `dashboard/src/pages/Report.tsx`

**Step 1: Create the component**

```tsx
import type { FileAge } from '../types'

interface Props { ages: FileAge[] }

function ageBand(days: number): { color: string; label: string } {
  if (days <= 30)  return { color: '#10b981', label: 'fresh' }
  if (days <= 90)  return { color: '#34d399', label: '< 3mo' }
  if (days <= 180) return { color: '#f59e0b', label: '< 6mo' }
  if (days <= 365) return { color: '#fb923c', label: '< 1yr' }
  return { color: '#ef4444', label: '> 1yr' }
}

export default function AgeView({ ages }: Props) {
  if (ages.length === 0) {
    return <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>No age data available.</p>
  }

  const maxDays = ages[0]?.days_since_modified ?? 1

  return (
    <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
      <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: '0.75rem' }}>
        Code age — sorted by staleness (oldest first)
      </p>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400, minWidth: 160 }}>Age</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Days</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Last modified</th>
          </tr>
        </thead>
        <tbody>
          {ages.map((f, i) => {
            const { color, label } = ageBand(f.days_since_modified)
            const pct = (f.days_since_modified / maxDays) * 100
            const date = new Date(f.last_modified).toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' })
            return (
              <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
                <td style={{ padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.8)', maxWidth: 260, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={f.path}>
                  {f.path.split('/').pop()}
                  <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>
                    {f.path.includes('/') ? f.path.substring(0, f.path.lastIndexOf('/') + 1) : ''}
                  </span>
                </td>
                <td style={{ padding: '0.4rem 0.5rem' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                    <div style={{ flex: 1, height: 4, backgroundColor: 'rgba(255,255,255,0.06)', borderRadius: 2 }}>
                      <div style={{ width: `${pct}%`, height: '100%', backgroundColor: color, borderRadius: 2, boxShadow: `0 0 4px ${color}40` }} />
                    </div>
                    <span style={{ color, fontSize: '0.65rem', minWidth: '2.5rem' }}>{label}</span>
                  </div>
                </td>
                <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.5)' }}>{f.days_since_modified}</td>
                <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.4)', fontSize: '0.65rem' }}>{date}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
```

**Step 2: Wire into Report.tsx**

```tsx
{activeTab === 'age' && <AgeView ages={report.file_ages} />}
```

**Step 3: Final build verification**

```bash
cd dashboard && pnpm run build 2>&1
```
Expected: `✓ built` with no errors

**Step 4: Regenerate sample and do a smoke test**

```bash
cd /path/to/barad-dur
cargo run --release -- analyze . --json > dashboard/sample.json
```

Open browser at `http://localhost:5174`, drop `sample.json`, click through all 5 tabs and verify data appears.

**Step 5: Run full Rust test suite one last time**

```bash
cargo test 2>&1 | tail -5
```
Expected: All tests pass

**Step 6: Final commit**

```bash
git add dashboard/src/components/AgeView.tsx dashboard/src/pages/Report.tsx
git commit -m "feat(dashboard): add file age view"
git add -A
git commit -m "feat: complete code-maat file analysis (hotspots, coupling, ownership, age)"
```

---

## Verification Checklist

- [ ] `cargo test` — all pass
- [ ] `cargo run -- analyze . --json | python3 -c "import json,sys; r=json.load(sys.stdin); assert len(r['file_hotspots']) > 0"` — passes
- [ ] `cd dashboard && pnpm run build` — no errors
- [ ] Drop sample.json on landing page — all 5 tabs render with real data
- [ ] Hotspot scatter plot shows bubbles
- [ ] Coupling table shows % bars
- [ ] Ownership table shows stacked author bars
- [ ] Age table shows color-banded staleness bars
- [ ] `cargo clippy -- -D warnings` — no warnings
