# Multi-Repo Dashboard Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `dashboard` subcommand that aggregates reports from multiple repositories into a single HTML comparison view, letting teams see health across all their services at a glance.

**Architecture:** New `Dashboard` variant in `Commands` enum, a `src/dashboard.rs` module for aggregation logic, and a `src/renderer/dashboard_html.rs` for the self-contained HTML template. Reads existing `.repository-analysis/history.json` from each repo (no re-analysis needed). Can also accept JSON report files as input.

**Tech Stack:** Rust, clap (CLI args), serde/serde_json (data model), chrono (timestamps), vanilla JS/CSS (HTML template)

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/cli.rs` | Modify | Add `Dashboard(DashboardArgs)` variant + `DashboardArgs` struct |
| `src/dashboard.rs` | Create | `DashboardEntry`, `Dashboard` structs, `collect_dashboard()` aggregation |
| `src/renderer/dashboard_html.rs` | Create | Self-contained HTML generation for multi-repo dashboard |
| `src/renderer/mod.rs` | Modify | Add `pub mod dashboard_html;` |
| `src/lib.rs` | Modify | Add `pub mod dashboard;` |
| `src/main.rs` | Modify | Wire `Commands::Dashboard` to `run_dashboard()` |

---

## Task 1: Add DashboardArgs to cli.rs

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Write failing tests for dashboard CLI parsing**

In `src/cli.rs`, in the existing `#[cfg(test)] mod tests` block, add these tests at the end (before the closing `}`):

```rust
fn parse_dashboard(args: &[&str]) -> DashboardArgs {
    let cli = Cli::parse_from(args);
    match cli.command {
        Commands::Dashboard(d) => d,
        _ => panic!("expected Dashboard command"),
    }
}

#[test]
fn dashboard_subcommand_parses() {
    let cli = Cli::parse_from(["barad-dur", "dashboard", "/tmp/repos/a", "/tmp/repos/b"]);
    assert!(matches!(cli.command, Commands::Dashboard(_)));
}

#[test]
fn dashboard_targets() {
    let args = parse_dashboard(&["barad-dur", "dashboard", "/tmp/a", "/tmp/b"]);
    assert_eq!(args.targets, vec!["/tmp/a", "/tmp/b"]);
}

#[test]
fn dashboard_reports_flag() {
    let args = parse_dashboard(&[
        "barad-dur", "dashboard", "--reports", "a.json", "b.json",
    ]);
    assert!(args.reports);
    assert_eq!(args.targets, vec!["a.json", "b.json"]);
}

#[test]
fn dashboard_json_flag() {
    let args = parse_dashboard(&["barad-dur", "dashboard", "--json", "/tmp/a"]);
    assert!(args.json);
}

#[test]
fn dashboard_open_flag() {
    let args = parse_dashboard(&["barad-dur", "dashboard", "--open", "/tmp/a"]);
    assert!(args.open);
}

#[test]
fn dashboard_output_flag() {
    let args = parse_dashboard(&[
        "barad-dur", "dashboard", "/tmp/a", "-o", "dash.html",
    ]);
    assert_eq!(args.output, Some(PathBuf::from("dash.html")));
}

#[test]
fn dashboard_no_targets_uses_default() {
    let args = parse_dashboard(&["barad-dur", "dashboard"]);
    assert!(args.targets.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib cli::tests::dashboard
```

Expected: compile error -- `DashboardArgs` not found, no `Dashboard` variant.

- [ ] **Step 3: Add DashboardArgs struct and Commands variant**

In `src/cli.rs`, add the `Dashboard` variant to the `Commands` enum:

```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze a git repository
    Analyze(AnalyzeArgs),
    /// Generate a .repository-analysis/barad-dur.toml configuration file
    Init(InitArgs),
    /// Aggregate reports from multiple repositories into a comparison dashboard
    Dashboard(DashboardArgs),
}
```

Add the `DashboardArgs` struct after `InitArgs`:

```rust
#[derive(clap::Args, Debug)]
#[command(
    about = "Aggregate multi-repo reports into a comparison dashboard",
    long_about = "Reads .repository-analysis/history.json from each target repository (or JSON report \
        files with --reports) and generates a single HTML page comparing health scores, \
        trends, and key metrics across all repositories.\n\n\
        No re-analysis is performed -- only existing cached data is used.",
    after_long_help = "\
EXAMPLES:\n    \
  barad-dur dashboard /path/to/repos/*                      # scan dirs for repos\n    \
  barad-dur dashboard --reports r1.json r2.json r3.json     # use JSON report files\n    \
  barad-dur dashboard /path/to/repos/* --open               # generate + open browser\n    \
  barad-dur dashboard /path/to/repos/* -o dashboard.html    # write to file\n    \
  barad-dur dashboard /path/to/repos/* --json               # machine-readable summary"
)]
pub struct DashboardArgs {
    /// Paths to repository directories or JSON report files
    #[arg(trailing_var_arg = true)]
    pub targets: Vec<String>,

    /// Treat targets as JSON report files instead of repo paths
    #[arg(long)]
    pub reports: bool,

    /// Output as JSON instead of HTML
    #[arg(long)]
    pub json: bool,

    /// Open the dashboard in the default browser after generation
    #[arg(long)]
    pub open: bool,

    /// Write output to a file instead of stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib cli::tests::dashboard
```

Expected: 7 tests pass.

- [ ] **Step 5: Run full cli test suite to check no regressions**

```bash
cargo test --lib cli::tests
```

Expected: all existing + new tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs
git commit -m "feat(cli): add Dashboard subcommand with DashboardArgs"
```

---

## Task 2: Create src/dashboard.rs with data model and collection logic

**Files:**
- Create: `src/dashboard.rs`
- Modify: `src/lib.rs` -- add `pub mod dashboard;`

- [ ] **Step 1: Write failing tests for DashboardEntry and Dashboard**

Create `src/dashboard.rs` with structs and test module:

```rust
use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::cache::history;
use crate::scorer::{AnalysisReport, HistoryEntry};

#[derive(Debug, Clone, Serialize)]
pub struct DashboardEntry {
    pub repo_name: String,
    pub path: String,
    pub overall_score: u32,
    pub categories: HashMap<String, u32>,
    pub total_files: usize,
    pub total_commits: usize,
    pub total_authors: usize,
    pub trend: Option<i32>,
    pub last_analyzed: Option<chrono::DateTime<chrono::Utc>>,
    pub history_len: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dashboard {
    pub entries: Vec<DashboardEntry>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

impl Dashboard {
    pub fn avg_score(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.entries.iter().map(|e| e.overall_score).sum();
        sum as f64 / self.entries.len() as f64
    }

    pub fn best(&self) -> Option<&DashboardEntry> {
        self.entries.iter().max_by_key(|e| e.overall_score)
    }

    pub fn worst(&self) -> Option<&DashboardEntry> {
        self.entries.iter().min_by_key(|e| e.overall_score)
    }
}

// TODO: entry_from_history, entry_from_report, collect_from_repos, collect_from_reports

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorer::HistoryCounts;

    fn make_history_entry(head: &str, score: u32, cats: &[(&str, u32)]) -> HistoryEntry {
        let categories: HashMap<String, u32> =
            cats.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        HistoryEntry {
            timestamp: chrono::Utc::now(),
            head: head.to_string(),
            overall_score: score,
            categories,
            metrics: HashMap::new(),
            counts: HistoryCounts {
                commits: 100,
                files: 50,
                authors: 5,
            },
        }
    }

    #[test]
    fn entry_from_history_uses_latest() {
        let entries = vec![
            make_history_entry("aaa", 60, &[("Health", 55)]),
            make_history_entry("bbb", 75, &[("Health", 80)]),
        ];
        let entry = entry_from_history(&entries, "/tmp/repo", "my-repo");
        assert_eq!(entry.overall_score, 75);
        assert_eq!(entry.categories.get("Health"), Some(&80));
        assert_eq!(entry.trend, Some(15)); // 75 - 60
        assert_eq!(entry.history_len, 2);
    }

    #[test]
    fn entry_from_history_single_entry_no_trend() {
        let entries = vec![make_history_entry("aaa", 60, &[])];
        let entry = entry_from_history(&entries, "/tmp/repo", "my-repo");
        assert_eq!(entry.overall_score, 60);
        assert_eq!(entry.trend, None);
    }

    #[test]
    fn entry_from_report_extracts_fields() {
        let report = make_test_report("test-repo", 72, &[("Health", 80), ("Team", 65)]);
        let entry = entry_from_report(&report, "/tmp/test-repo.json");
        assert_eq!(entry.repo_name, "test-repo");
        assert_eq!(entry.overall_score, 72);
        assert_eq!(entry.categories.get("Health"), Some(&80));
        assert_eq!(entry.total_files, 50);
    }

    #[test]
    fn dashboard_avg_score() {
        let d = Dashboard {
            entries: vec![
                make_dashboard_entry("a", 80),
                make_dashboard_entry("b", 60),
            ],
            generated_at: chrono::Utc::now(),
        };
        assert!((d.avg_score() - 70.0).abs() < 0.01);
    }

    #[test]
    fn dashboard_best_worst() {
        let d = Dashboard {
            entries: vec![
                make_dashboard_entry("a", 80),
                make_dashboard_entry("b", 60),
                make_dashboard_entry("c", 90),
            ],
            generated_at: chrono::Utc::now(),
        };
        assert_eq!(d.best().unwrap().repo_name, "c");
        assert_eq!(d.worst().unwrap().repo_name, "b");
    }

    #[test]
    fn dashboard_empty() {
        let d = Dashboard {
            entries: vec![],
            generated_at: chrono::Utc::now(),
        };
        assert!((d.avg_score() - 0.0).abs() < 0.01);
        assert!(d.best().is_none());
        assert!(d.worst().is_none());
    }

    #[test]
    fn collect_from_repos_skips_missing_history() {
        let dir = tempfile::TempDir::new().unwrap();
        // Create a dir with no .repository-analysis/history.json
        let repo_dir = dir.path().join("empty-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let dashboard = collect_from_repos(&[repo_dir.to_string_lossy().to_string()]);
        assert!(dashboard.entries.is_empty());
    }

    #[test]
    fn collect_from_repos_loads_history() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo_dir = dir.path().join("my-repo");
        let cache_dir = repo_dir.join(".repository-analysis");
        std::fs::create_dir_all(&cache_dir).unwrap();

        // Write a history entry
        let entry = make_history_entry("abc", 72, &[("Health", 80)]);
        crate::cache::history::append_if_new_head(&entry, &repo_dir).unwrap();

        let dashboard = collect_from_repos(&[repo_dir.to_string_lossy().to_string()]);
        assert_eq!(dashboard.entries.len(), 1);
        assert_eq!(dashboard.entries[0].overall_score, 72);
    }

    // -- test helpers --

    fn make_dashboard_entry(name: &str, score: u32) -> DashboardEntry {
        DashboardEntry {
            repo_name: name.to_string(),
            path: format!("/tmp/{}", name),
            overall_score: score,
            categories: HashMap::new(),
            total_files: 0,
            total_commits: 0,
            total_authors: 0,
            trend: None,
            last_analyzed: None,
            history_len: 0,
        }
    }

    fn make_test_report(
        name: &str,
        score: u32,
        cats: &[(&str, u32)],
    ) -> AnalysisReport {
        use crate::metrics::{CategoryResult, MetricValue, RawValue};
        AnalysisReport {
            repo_name: name.into(),
            branch: "main".into(),
            time_window_months: 6,
            total_commits: 100,
            total_authors: 5,
            total_files: 50,
            overall_score: score,
            categories: cats
                .iter()
                .map(|(n, s)| CategoryResult {
                    name: n.to_string(),
                    score: *s,
                    metrics: vec![MetricValue {
                        name: format!("{} metric", n),
                        description: "test".into(),
                        raw_value: RawValue::Integer(0),
                        score: *s,
                    }],
                })
                .collect(),
            top_actions: vec![],
            remote_meta: None,
            file_hotspots: vec![],
            coupling_pairs: vec![],
            author_ownership: vec![],
            file_ages: vec![],
            history: vec![],
        }
    }
}
```

- [ ] **Step 2: Add `pub mod dashboard;` to `src/lib.rs`**

In `src/lib.rs`, add after the existing modules:

```rust
pub mod dashboard;
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test --lib dashboard::tests
```

Expected: compile errors -- `entry_from_history`, `entry_from_report`, `collect_from_repos` not found.

- [ ] **Step 4: Implement `entry_from_history`**

In `src/dashboard.rs`, replace the `// TODO` comment with:

```rust
/// Build a DashboardEntry from a repo's history entries (latest wins).
pub fn entry_from_history(entries: &[HistoryEntry], path: &str, repo_name: &str) -> DashboardEntry {
    let latest = entries.last().expect("entries must not be empty");
    let trend = if entries.len() >= 2 {
        let prev = &entries[entries.len() - 2];
        Some(latest.overall_score as i32 - prev.overall_score as i32)
    } else {
        None
    };

    DashboardEntry {
        repo_name: repo_name.to_string(),
        path: path.to_string(),
        overall_score: latest.overall_score,
        categories: latest.categories.clone(),
        total_files: latest.counts.files,
        total_commits: latest.counts.commits,
        total_authors: latest.counts.authors,
        trend,
        last_analyzed: Some(latest.timestamp),
        history_len: entries.len(),
    }
}
```

- [ ] **Step 5: Implement `entry_from_report`**

```rust
/// Build a DashboardEntry from a full AnalysisReport (JSON file input).
pub fn entry_from_report(report: &AnalysisReport, path: &str) -> DashboardEntry {
    let categories: HashMap<String, u32> = report
        .categories
        .iter()
        .map(|c| (c.name.clone(), c.score))
        .collect();

    // Compute trend from embedded history if available
    let trend = if report.history.len() >= 2 {
        let prev = &report.history[report.history.len() - 2];
        Some(report.overall_score as i32 - prev.overall_score as i32)
    } else {
        None
    };

    let last_analyzed = report.history.last().map(|h| h.timestamp);

    DashboardEntry {
        repo_name: report.repo_name.clone(),
        path: path.to_string(),
        overall_score: report.overall_score,
        categories,
        total_files: report.total_files,
        total_commits: report.total_commits,
        total_authors: report.total_authors,
        trend,
        last_analyzed,
        history_len: report.history.len(),
    }
}
```

- [ ] **Step 6: Implement `collect_from_repos`**

```rust
/// Scan repo directories and build a Dashboard from their history files.
pub fn collect_from_repos(targets: &[String]) -> Dashboard {
    let mut entries = Vec::new();

    for target in targets {
        let path = Path::new(target);
        if !path.is_dir() {
            eprintln!("Warning: skipping {} (not a directory)", target);
            continue;
        }

        match history::load_history(path) {
            Ok(hist) if !hist.is_empty() => {
                let repo_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| target.clone());
                entries.push(entry_from_history(&hist, target, &repo_name));
            }
            Ok(_) => {
                eprintln!("Warning: skipping {} (no history data)", target);
            }
            Err(e) => {
                eprintln!("Warning: skipping {} ({})", target, e);
            }
        }
    }

    // Sort by score descending (best first)
    entries.sort_by(|a, b| b.overall_score.cmp(&a.overall_score));

    Dashboard {
        entries,
        generated_at: chrono::Utc::now(),
    }
}
```

- [ ] **Step 7: Implement `collect_from_reports`**

```rust
/// Build a Dashboard from JSON report files.
pub fn collect_from_reports(paths: &[String]) -> Dashboard {
    let mut entries = Vec::new();

    for path in paths {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<AnalysisReport>(&content) {
                Ok(report) => {
                    entries.push(entry_from_report(&report, path));
                }
                Err(e) => {
                    eprintln!("Warning: skipping {} (invalid JSON: {})", path, e);
                }
            },
            Err(e) => {
                eprintln!("Warning: skipping {} ({})", path, e);
            }
        }
    }

    entries.sort_by(|a, b| b.overall_score.cmp(&a.overall_score));

    Dashboard {
        entries,
        generated_at: chrono::Utc::now(),
    }
}
```

- [ ] **Step 8: Run tests to verify they pass**

```bash
cargo test --lib dashboard::tests
```

Expected: all 8 tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/dashboard.rs src/lib.rs
git commit -m "feat: add Dashboard data model and collection logic"
```

---

## Task 3: Create src/renderer/dashboard_html.rs

**Files:**
- Create: `src/renderer/dashboard_html.rs`
- Modify: `src/renderer/mod.rs` -- add `pub mod dashboard_html;`

- [ ] **Step 1: Write failing tests for dashboard HTML rendering**

Create `src/renderer/dashboard_html.rs`:

```rust
use anyhow::Result;

use crate::dashboard::Dashboard;

/// Render the multi-repo dashboard as a self-contained HTML file.
/// All CSS, JS, and data are inlined. No external dependencies.
pub fn render(dashboard: &Dashboard) -> Result<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::DashboardEntry;
    use std::collections::HashMap;

    fn make_entry(name: &str, score: u32, trend: Option<i32>) -> DashboardEntry {
        let mut categories = HashMap::new();
        categories.insert("Health".to_string(), score);
        categories.insert("Team".to_string(), score.saturating_sub(10));
        categories.insert("Evolution".to_string(), score.saturating_sub(5));
        categories.insert("Git Hygiene".to_string(), score.saturating_add(5).min(100));
        DashboardEntry {
            repo_name: name.to_string(),
            path: format!("/tmp/{}", name),
            overall_score: score,
            categories,
            total_files: 50,
            total_commits: 100,
            total_authors: 5,
            trend,
            last_analyzed: Some(chrono::Utc::now()),
            history_len: 3,
        }
    }

    fn make_dashboard() -> Dashboard {
        Dashboard {
            entries: vec![
                make_entry("service-a", 82, Some(5)),
                make_entry("service-b", 65, Some(-3)),
                make_entry("service-c", 45, None),
            ],
            generated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn html_is_self_contained() {
        let html = render(&make_dashboard()).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<style>"));
        assert!(html.contains("<script>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn html_contains_dashboard_title() {
        let html = render(&make_dashboard()).unwrap();
        assert!(html.contains("Barad-d"));
        assert!(html.contains("Dashboard"));
    }

    #[test]
    fn html_embeds_data_in_window_d() {
        let html = render(&make_dashboard()).unwrap();
        assert!(html.contains("window.D="));
    }

    #[test]
    fn html_contains_repo_names() {
        let html = render(&make_dashboard()).unwrap();
        assert!(html.contains("service-a"));
        assert!(html.contains("service-b"));
        assert!(html.contains("service-c"));
    }

    #[test]
    fn html_contains_summary_bar() {
        let html = render(&make_dashboard()).unwrap();
        assert!(html.contains("summary"));
    }

    #[test]
    fn html_contains_sortable_table() {
        let html = render(&make_dashboard()).unwrap();
        assert!(html.contains("sortable") || html.contains("sort"));
    }

    #[test]
    fn html_contains_search_filter() {
        let html = render(&make_dashboard()).unwrap();
        assert!(html.contains("search") || html.contains("filter"));
    }

    #[test]
    fn html_empty_dashboard_shows_message() {
        let d = Dashboard {
            entries: vec![],
            generated_at: chrono::Utc::now(),
        };
        let html = render(&d).unwrap();
        assert!(html.contains("no repo") || html.contains("No repo") || html.contains("empty"));
    }
}
```

- [ ] **Step 2: Add `pub mod dashboard_html;` to `src/renderer/mod.rs`**

```rust
pub mod cli;
pub mod dashboard_html;
pub mod html;
pub mod json;
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test --lib renderer::dashboard_html::tests
```

Expected: FAIL -- `todo!()` panics at runtime.

- [ ] **Step 4: Implement the CSS constant**

In `src/renderer/dashboard_html.rs`, add before `render`:

```rust
const CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #080a0f;
  color: #e2e8f0;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  font-size: 14px;
  min-height: 100vh;
}
header {
  background: #0d1117;
  border-bottom: 1px solid #1e293b;
  padding: 16px 24px;
}
.header-row {
  display: flex; align-items: center; justify-content: space-between;
  flex-wrap: wrap; gap: 12px;
}
.brand { font-size: 20px; font-weight: 700; color: #f8fafc; }
.brand em { color: #f97316; font-style: normal; }
.meta { color: #8b949e; font-size: 13px; }
.summary-bar {
  display: flex; gap: 16px; padding: 16px 24px;
  background: #0d1117; border-bottom: 1px solid #1e293b;
  flex-wrap: wrap;
}
.summary-card {
  background: #161b22; border-radius: 8px; padding: 12px 20px;
  min-width: 160px; flex: 1;
}
.summary-label { color: #8b949e; font-size: 11px; text-transform: uppercase;
  letter-spacing: 0.06em; margin-bottom: 4px; }
.summary-value { font-size: 24px; font-weight: 700; }
.content { padding: 16px 24px; max-width: 1400px; margin: 0 auto; }
.controls { display: flex; gap: 12px; margin-bottom: 16px; align-items: center; }
.search-input {
  background: #161b22; color: #c9d1d9; border: 1px solid #1e293b;
  border-radius: 6px; padding: 8px 12px; font-size: 14px; width: 260px;
}
.search-input::placeholder { color: #484f58; }
table { width: 100%; border-collapse: collapse; }
th {
  text-align: left; padding: 10px 12px; color: #8b949e; font-size: 11px;
  text-transform: uppercase; letter-spacing: 0.06em;
  border-bottom: 1px solid #1e293b; cursor: pointer; user-select: none;
  white-space: nowrap;
}
th:hover { color: #c9d1d9; }
th .sort-arrow { margin-left: 4px; font-size: 10px; }
td { padding: 10px 12px; border-bottom: 1px solid #111827; }
tr:hover td { background: #161b22; }
.score-pill {
  display: inline-block; padding: 2px 10px; border-radius: 12px;
  font-weight: 600; font-size: 13px; min-width: 40px; text-align: center;
}
.trend-up { color: #10b981; }
.trend-down { color: #ef4444; }
.trend-flat { color: #8b949e; }
.repo-link { color: #58a6ff; text-decoration: none; }
.repo-link:hover { text-decoration: underline; }
.empty-state {
  text-align: center; color: #8b949e; padding: 80px 20px; font-size: 16px;
}
"#;
```

- [ ] **Step 5: Implement the JS builder function**

Add a `build_js()` function. The JS renders the dashboard using safe DOM manipulation for user-facing text. Note: the JS uses `el.textContent` for repo names and other user-provided strings to prevent injection. The table rendering uses DOM methods (`createElement`, `appendChild`, `textContent`) rather than string interpolation for all cells containing user data:

```rust
fn build_js() -> String {
    r#"
(function() {
  var D = window.D;
  var app = document.getElementById('app');

  function scoreColor(s) {
    if (s >= 71) return '#10b981';
    if (s >= 41) return '#f59e0b';
    return '#ef4444';
  }

  function scoreBg(s) {
    if (s >= 71) return 'rgba(16,185,129,0.15)';
    if (s >= 41) return 'rgba(245,158,11,0.15)';
    return 'rgba(239,68,68,0.15)';
  }

  function buildScorePill(score) {
    var span = document.createElement('span');
    span.className = 'score-pill';
    span.style.color = scoreColor(score);
    span.style.background = scoreBg(score);
    span.textContent = score;
    return span;
  }

  function buildTrend(t) {
    var span = document.createElement('span');
    if (t == null) { span.className = 'trend-flat'; span.textContent = '--'; }
    else if (t > 0) { span.className = 'trend-up'; span.textContent = '\u25B2 +' + t; }
    else if (t < 0) { span.className = 'trend-down'; span.textContent = '\u25BC ' + t; }
    else { span.className = 'trend-flat'; span.textContent = '\u2500 0'; }
    return span;
  }

  function buildCatScore(entry, name) {
    var s = entry.categories[name];
    if (s == null) {
      var span = document.createElement('span');
      span.style.color = '#484f58';
      span.textContent = '--';
      return span;
    }
    return buildScorePill(s);
  }

  // -- Summary bar (safe: only numeric and our own repo names via textContent) --
  function buildSummary() {
    var entries = D.entries;
    if (!entries.length) return document.createDocumentFragment();
    var sum = 0; var best = entries[0]; var worst = entries[0];
    entries.forEach(function(e) {
      sum += e.overall_score;
      if (e.overall_score > best.overall_score) best = e;
      if (e.overall_score < worst.overall_score) worst = e;
    });
    var avg = Math.round(sum / entries.length);

    var bar = document.createElement('div');
    bar.className = 'summary-bar';

    function card(label, value, color) {
      var c = document.createElement('div'); c.className = 'summary-card';
      var l = document.createElement('div'); l.className = 'summary-label'; l.textContent = label;
      var v = document.createElement('div'); v.className = 'summary-value';
      v.textContent = value; if (color) v.style.color = color;
      c.appendChild(l); c.appendChild(v); return c;
    }

    bar.appendChild(card('Repositories', entries.length, null));
    bar.appendChild(card('Average Score', avg, scoreColor(avg)));
    bar.appendChild(card('Best', best.repo_name + ' (' + best.overall_score + ')', scoreColor(best.overall_score)));
    bar.appendChild(card('Worst', worst.repo_name + ' (' + worst.overall_score + ')', scoreColor(worst.overall_score)));
    return bar;
  }

  // -- Sortable table --
  var sortCol = 'overall_score';
  var sortAsc = false;

  function sortEntries(entries) {
    var col = sortCol;
    return entries.slice().sort(function(a, b) {
      var va, vb;
      if (col === 'repo_name') { va = a.repo_name.toLowerCase(); vb = b.repo_name.toLowerCase(); }
      else if (col === 'overall_score') { va = a.overall_score; vb = b.overall_score; }
      else if (col === 'trend') { va = a.trend || 0; vb = b.trend || 0; }
      else if (col === 'total_files') { va = a.total_files; vb = b.total_files; }
      else if (col === 'total_commits') { va = a.total_commits; vb = b.total_commits; }
      else if (col === 'total_authors') { va = a.total_authors; vb = b.total_authors; }
      else { va = (a.categories[col] || 0); vb = (b.categories[col] || 0); }
      if (va < vb) return sortAsc ? -1 : 1;
      if (va > vb) return sortAsc ? 1 : -1;
      return 0;
    });
  }

  var cats = ['Health', 'Team', 'Evolution', 'Git Hygiene'];

  function buildTable(filter) {
    var entries = D.entries;
    if (filter) {
      var q = filter.toLowerCase();
      entries = entries.filter(function(e) { return e.repo_name.toLowerCase().indexOf(q) >= 0; });
    }
    entries = sortEntries(entries);

    var table = document.createElement('table');
    var thead = document.createElement('thead');
    var headerRow = document.createElement('tr');

    var columns = [
      { col: 'repo_name', label: 'Repo' },
      { col: 'overall_score', label: 'Score' }
    ];
    cats.forEach(function(c) { columns.push({ col: c, label: c }); });
    columns.push({ col: 'trend', label: 'Trend' });
    columns.push({ col: 'total_files', label: 'Files' });
    columns.push({ col: 'total_commits', label: 'Commits' });
    columns.push({ col: 'total_authors', label: 'Authors' });

    columns.forEach(function(c) {
      var th = document.createElement('th');
      th.setAttribute('data-col', c.col);
      th.textContent = c.label;
      if (c.col === sortCol) {
        var arrow = document.createElement('span');
        arrow.className = 'sort-arrow';
        arrow.textContent = sortAsc ? '\u25B2' : '\u25BC';
        th.appendChild(arrow);
      }
      headerRow.appendChild(th);
    });
    thead.appendChild(headerRow);
    table.appendChild(thead);

    var tbody = document.createElement('tbody');
    entries.forEach(function(e) {
      var tr = document.createElement('tr');

      var tdName = document.createElement('td');
      var nameSpan = document.createElement('span');
      nameSpan.className = 'repo-link';
      nameSpan.textContent = e.repo_name;
      tdName.appendChild(nameSpan);
      tr.appendChild(tdName);

      var tdScore = document.createElement('td');
      tdScore.appendChild(buildScorePill(e.overall_score));
      tr.appendChild(tdScore);

      cats.forEach(function(c) {
        var td = document.createElement('td');
        td.appendChild(buildCatScore(e, c));
        tr.appendChild(td);
      });

      var tdTrend = document.createElement('td');
      tdTrend.appendChild(buildTrend(e.trend));
      tr.appendChild(tdTrend);

      ['total_files', 'total_commits', 'total_authors'].forEach(function(key) {
        var td = document.createElement('td');
        td.textContent = e[key];
        tr.appendChild(td);
      });

      tbody.appendChild(tr);
    });
    table.appendChild(tbody);
    return table;
  }

  function bindSortHeaders(container) {
    var ths = container.querySelectorAll('th[data-col]');
    ths.forEach(function(th) {
      th.addEventListener('click', function() {
        var col = this.getAttribute('data-col');
        if (sortCol === col) { sortAsc = !sortAsc; }
        else { sortCol = col; sortAsc = false; }
        refreshTable();
      });
    });
  }

  var searchInput;
  var tableContainer;

  function refreshTable() {
    var q = searchInput ? searchInput.value : '';
    tableContainer.replaceChildren(buildTable(q));
    bindSortHeaders(tableContainer);
  }

  // -- Render --
  function renderApp() {
    app.replaceChildren(); // clear

    if (!D.entries.length) {
      var empty = document.createElement('div');
      empty.className = 'empty-state';
      empty.textContent = 'No repositories with analysis data found. Run barad-dur analyze on each repo first.';
      app.appendChild(empty);
      return;
    }

    // Header
    var header = document.createElement('header');
    var headerRow = document.createElement('div');
    headerRow.className = 'header-row';
    var brand = document.createElement('span');
    brand.className = 'brand';
    brand.textContent = 'Barad-d';
    var em = document.createElement('em');
    em.textContent = '\u00FB';
    brand.appendChild(em);
    var brandSuffix = document.createTextNode('r Dashboard');
    brand.appendChild(brandSuffix);
    headerRow.appendChild(brand);
    var meta = document.createElement('span');
    meta.className = 'meta';
    meta.textContent = D.entries.length + ' repositories \u00B7 ' + new Date(D.generated_at).toLocaleString();
    headerRow.appendChild(meta);
    header.appendChild(headerRow);
    app.appendChild(header);

    // Summary
    app.appendChild(buildSummary());

    // Content
    var content = document.createElement('div');
    content.className = 'content';

    var controls = document.createElement('div');
    controls.className = 'controls';
    searchInput = document.createElement('input');
    searchInput.type = 'text';
    searchInput.className = 'search-input';
    searchInput.placeholder = 'Filter by repo name...';
    searchInput.addEventListener('input', function() { refreshTable(); });
    controls.appendChild(searchInput);
    content.appendChild(controls);

    tableContainer = document.createElement('div');
    tableContainer.id = 'table-container';
    tableContainer.appendChild(buildTable(''));
    bindSortHeaders(tableContainer);
    content.appendChild(tableContainer);

    app.appendChild(content);
  }

  renderApp();
})();
"#.to_string()
}
```

- [ ] **Step 6: Implement the `render` function**

Replace the `todo!()` in `render`:

```rust
pub fn render(dashboard: &Dashboard) -> Result<String> {
    let json = serde_json::to_string(dashboard)?;
    let json = json.replace("</", "<\\/");

    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
        <title>Barad-d\u{00fb}r Dashboard</title>\n<style>\n{css}\n</style>\n</head>\n<body>\n\
        <script>window.D={json};</script>\n\
        <div id=\"app\"></div>\n\
        <script>\n{js}\n</script>\n</body>\n</html>",
        css = CSS,
        json = json,
        js = build_js(),
    );
    Ok(html)
}
```

- [ ] **Step 7: Run tests to verify they pass**

```bash
cargo test --lib renderer::dashboard_html::tests
```

Expected: all 8 tests pass.

- [ ] **Step 8: Run full test suite**

```bash
cargo test --lib
```

Expected: all tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/renderer/dashboard_html.rs src/renderer/mod.rs
git commit -m "feat: add self-contained HTML dashboard renderer"
```

---

## Task 4: Wire into main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add the `Commands::Dashboard` match arm**

In `src/main.rs`, update the `match cli.command` block:

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => run_analyze(args)?,
        Commands::Init(args) => {
            let target = std::path::PathBuf::from(&args.target);
            barad_dur::init::run_init(&target, args.force, args.interactive)?;
        }
        Commands::Dashboard(args) => run_dashboard(args)?,
    }
    Ok(())
}
```

- [ ] **Step 2: Implement `run_dashboard`**

Add to `src/main.rs`:

```rust
fn run_dashboard(args: barad_dur::cli::DashboardArgs) -> Result<()> {
    let dashboard = if args.reports {
        barad_dur::dashboard::collect_from_reports(&args.targets)
    } else {
        barad_dur::dashboard::collect_from_repos(&args.targets)
    };

    if args.json {
        let output = serde_json::to_string_pretty(&dashboard)?;
        if let Some(path) = &args.output {
            std::fs::write(path, &output)?;
            eprintln!("Dashboard JSON written to {}", path.display());
        } else {
            println!("{}", output);
        }
        return Ok(());
    }

    let output = barad_dur::renderer::dashboard_html::render(&dashboard)?;

    if args.open {
        let path = if let Some(ref p) = args.output {
            std::fs::write(p, &output)?;
            p.clone()
        } else {
            let tmp = std::env::temp_dir().join("barad-dur-dashboard.html");
            std::fs::write(&tmp, &output)?;
            tmp
        };
        eprintln!("Opening {}", path.display());
        open_in_browser(&path)?;
    } else if let Some(path) = &args.output {
        std::fs::write(path, &output)?;
        eprintln!("Dashboard written to {}", path.display());
    } else {
        print!("{}", output);
    }

    Ok(())
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo build
```

Expected: compiles without errors.

- [ ] **Step 4: Run full test suite**

```bash
cargo test --lib
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire dashboard subcommand into main"
```

---

## Task 5: Add JSON output support

**Files:**
- Modify: `src/dashboard.rs` (ensure Serialize derives are complete)

- [ ] **Step 1: Write test for JSON serialization**

In `src/dashboard.rs` tests, add:

```rust
#[test]
fn dashboard_serializes_to_json() {
    let d = Dashboard {
        entries: vec![make_dashboard_entry("test-repo", 75)],
        generated_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&d).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["entries"].is_array());
    assert_eq!(parsed["entries"][0]["repo_name"], "test-repo");
    assert_eq!(parsed["entries"][0]["overall_score"], 75);
    assert!(parsed["generated_at"].is_string());
}

#[test]
fn dashboard_entry_json_includes_all_fields() {
    let d = Dashboard {
        entries: vec![DashboardEntry {
            repo_name: "my-service".to_string(),
            path: "/repos/my-service".to_string(),
            overall_score: 82,
            categories: {
                let mut m = HashMap::new();
                m.insert("Health".to_string(), 90);
                m
            },
            total_files: 200,
            total_commits: 500,
            total_authors: 12,
            trend: Some(5),
            last_analyzed: Some(chrono::Utc::now()),
            history_len: 7,
        }],
        generated_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string_pretty(&d).unwrap();
    assert!(json.contains("\"trend\": 5"));
    assert!(json.contains("\"history_len\": 7"));
    assert!(json.contains("\"total_authors\": 12"));
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test --lib dashboard::tests::dashboard_serializes
cargo test --lib dashboard::tests::dashboard_entry_json
```

Expected: both pass (Serialize is already derived). If not, add missing derives.

- [ ] **Step 3: Commit**

```bash
git add src/dashboard.rs
git commit -m "test: add JSON serialization tests for Dashboard"
```

---

## Task 6: Manual verification

- [ ] **Step 1: Build and install**

```bash
cargo install --path .
```

- [ ] **Step 2: Verify help text**

```bash
barad-dur dashboard --help
```

Expected: shows DashboardArgs with examples, `--reports`, `--json`, `--open`, `-o` flags.

- [ ] **Step 3: Run on repos with history data**

First ensure at least 2 repos have `.repository-analysis/history.json`:

```bash
# Check which repos have history
ls /home/edouard/WS/FW.All/repos/FW.Runtime/.repository-analysis/history.json
ls /home/edouard/WS/tool/myTool/.repository-analysis/history.json
```

If a repo lacks history, run `barad-dur analyze .` in it first.

```bash
barad-dur dashboard \
  /home/edouard/WS/FW.All/repos/FW.Runtime \
  /home/edouard/WS/tool/myTool \
  --open
```

Expected: browser opens a dashboard HTML page with:
- Header showing "Barad-dur Dashboard" and repo count
- Summary bar: avg score, best repo, worst repo
- Sortable table with both repos listed
- Clicking column headers sorts the table
- Search box filters by repo name

- [ ] **Step 4: Test JSON output**

```bash
barad-dur dashboard \
  /home/edouard/WS/FW.All/repos/FW.Runtime \
  /home/edouard/WS/tool/myTool \
  --json | python3 -m json.tool | head -30
```

Expected: valid JSON with `entries` array and `generated_at` timestamp.

- [ ] **Step 5: Test --reports mode**

```bash
# First generate a JSON report
barad-dur analyze /home/edouard/WS/tool/myTool --json --pretty -o /tmp/barad-report.json

# Then use it as dashboard input
barad-dur dashboard --reports /tmp/barad-report.json -o /tmp/dash-from-reports.html
```

Expected: dashboard HTML written to `/tmp/dash-from-reports.html` with one entry.

- [ ] **Step 6: Test with file output**

```bash
barad-dur dashboard \
  /home/edouard/WS/FW.All/repos/FW.Runtime \
  /home/edouard/WS/tool/myTool \
  -o /tmp/multi-dash.html
```

Expected: file written, stderr message confirms path.

- [ ] **Step 7: Test empty input (no matching repos)**

```bash
barad-dur dashboard /tmp
```

Expected: HTML output with empty state message ("No repositories with analysis data found").

- [ ] **Step 8: Run cargo fmt + clippy**

```bash
cargo fmt
cargo clippy -- -D warnings
```

Fix any warnings.

- [ ] **Step 9: Final commit and push**

```bash
git add -A
git commit -m "style: cargo fmt + clippy fix"
git push
```
