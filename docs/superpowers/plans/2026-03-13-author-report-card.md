# Per-Author Report Card — HTML Tab Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an "Authors" tab to the HTML report showing per-contributor metrics: files owned, bus factor exposure, commit quality, activity trend, and knowledge breadth.

**Architecture:** New `AuthorCard` struct in `src/scorer.rs`, computed from existing snapshot data (blame_map, commits, authors). A new `buildAuthorsTab()` JS function renders a sortable, filterable card grid. No new Rust metric modules needed — everything derives from existing data.

**Tech Stack:** Rust (scorer), serde_json (serialization into `window.R`), vanilla JS/DOM (tab rendering)

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/scorer.rs` | Modify | Add `AuthorCard` struct, `build_author_cards()` function, wire into `AnalysisReport` and `build_report()` |
| `src/renderer/html.rs` | Modify | Add Authors CSS, `buildAuthorsTab()` JS function, register tab in `renderApp()`, update `make_report()` and `make_treemap_report()` test helpers |

---

## Chunk 1: AuthorCard Struct + build_author_cards()

### Task 1: Add `AuthorCard` struct and `author_cards` field to `AnalysisReport`

**Files:**
- Modify: `src/scorer.rs:17-83` (structs and AnalysisReport)

- [ ] **Step 1: Write failing test — author_cards field exists on AnalysisReport**

In `src/scorer.rs` tests section, add:

```rust
#[test]
fn build_report_has_author_cards_field() {
    let snapshot = RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test-repo".into(),
        "main".into(),
        TimeWindow::default(),
    );
    let categories = vec![make_category("Health", 80)];
    let report = build_report(&snapshot, categories, None, WEIGHTS);
    // Field must exist and be empty for empty snapshot
    assert!(report.author_cards.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib build_report_has_author_cards_field`
Expected: compile error — `no field author_cards on type AnalysisReport`

- [ ] **Step 3: Add `AuthorCard` struct and `author_cards` field**

In `src/scorer.rs`, add the struct after `FileAge` (before `RemoteMeta`, around line 55):

```rust
#[derive(Debug, Clone, Serialize)]
pub struct AuthorCard {
    pub name: String,
    pub email: String,
    pub commit_count: usize,
    pub files_owned: usize,           // files where author has >50% blame
    pub lines_owned: usize,           // total blame lines across all files
    pub avg_commit_quality: f64,      // 0-100 based on message length/format
    pub top_files: Vec<String>,       // top 5 files by blame %
    pub last_active: chrono::DateTime<chrono::Utc>,
    pub days_since_active: i64,
    pub directories_touched: usize,   // breadth metric
}
```

In `AnalysisReport` (line 66-83), add field after `file_ages`:

```rust
pub author_cards: Vec<AuthorCard>,
```

- [ ] **Step 4: Fix all compilation errors from new field**

Update every place that constructs `AnalysisReport`:

In `build_report()` (line 140-157), add `author_cards: Vec::new(),` after `file_ages,` (placeholder — wired in Task 2).

In `src/renderer/html.rs` `make_report()` (line 2604-2631), add `author_cards: vec![],` after `file_ages: vec![],`.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib build_report_has_author_cards_field`
Expected: pass

---

### Task 2: Implement `build_author_cards()` and wire into `build_report()`

**Files:**
- Modify: `src/scorer.rs` (add function, update `build_report()`)

- [ ] **Step 1: Write failing test — build_author_cards produces correct output**

In `src/scorer.rs` tests section, add:

```rust
#[test]
fn build_author_cards_from_snapshot() {
    use crate::snapshot::*;
    use chrono::{Duration, Utc};

    let now = Utc::now();
    let mut snapshot = RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "t".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.authors = vec![
        Author { id: 0, name: "Alice".into(), email: "alice@x.com".into() },
        Author { id: 1, name: "Bob".into(), email: "bob@x.com".into() },
    ];
    snapshot.commits = vec![
        Commit {
            id: "c1".into(),
            author: 0,
            timestamp: now - Duration::days(10),
            message: "feat: add login flow with validation".into(),
            files_changed: vec![
                FileChange { path: "src/auth.rs".into(), additions: 50, deletions: 0, change_type: ChangeType::Modified },
                FileChange { path: "src/main.rs".into(), additions: 5, deletions: 0, change_type: ChangeType::Modified },
            ],
            is_merge: false,
            parent_count: 1,
        },
        Commit {
            id: "c2".into(),
            author: 0,
            timestamp: now - Duration::days(5),
            message: "fix: handle edge case in auth".into(),
            files_changed: vec![
                FileChange { path: "src/auth.rs".into(), additions: 10, deletions: 2, change_type: ChangeType::Modified },
            ],
            is_merge: false,
            parent_count: 1,
        },
        Commit {
            id: "c3".into(),
            author: 1,
            timestamp: now - Duration::days(100),
            message: "wip".into(),
            files_changed: vec![
                FileChange { path: "src/main.rs".into(), additions: 3, deletions: 1, change_type: ChangeType::Modified },
            ],
            is_merge: false,
            parent_count: 1,
        },
    ];
    // Blame: Alice owns 80% of auth.rs, Bob owns 60% of main.rs
    snapshot.blame_map.insert("src/auth.rs".into(), vec![
        BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now },
        BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now },
        BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now },
        BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now },
        BlameLine { author_id: 1, commit_id: "c3".into(), timestamp: now },
    ]);
    snapshot.blame_map.insert("src/main.rs".into(), vec![
        BlameLine { author_id: 1, commit_id: "c3".into(), timestamp: now },
        BlameLine { author_id: 1, commit_id: "c3".into(), timestamp: now },
        BlameLine { author_id: 1, commit_id: "c3".into(), timestamp: now },
        BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now },
        BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now },
    ]);
    snapshot.build_indexes();

    let cards = build_author_cards(&snapshot);
    assert_eq!(cards.len(), 2);

    let alice = cards.iter().find(|c| c.name == "Alice").unwrap();
    assert_eq!(alice.commit_count, 2);
    assert_eq!(alice.files_owned, 1); // >50% of auth.rs
    assert_eq!(alice.lines_owned, 6); // 4 in auth.rs + 2 in main.rs
    assert!(alice.avg_commit_quality > 50.0); // good messages
    assert!(alice.days_since_active < 30);
    assert_eq!(alice.directories_touched, 1); // only "src"

    let bob = cards.iter().find(|c| c.name == "Bob").unwrap();
    assert_eq!(bob.commit_count, 1);
    assert_eq!(bob.files_owned, 1); // >50% of main.rs
    assert_eq!(bob.lines_owned, 4); // 1 in auth.rs + 3 in main.rs
    assert!(bob.avg_commit_quality < 30.0); // "wip" is a bad message
    assert!(bob.days_since_active > 90);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib build_author_cards_from_snapshot`
Expected: compile error — `cannot find function build_author_cards`

- [ ] **Step 3: Implement `build_author_cards()`**

In `src/scorer.rs`, add after `build_file_ages()` (around line 306):

```rust
fn build_author_cards(snapshot: &RepoSnapshot) -> Vec<AuthorCard> {
    let now = chrono::Utc::now();

    // Pre-compute per-author blame lines across all files
    let mut author_lines: HashMap<usize, usize> = HashMap::new();
    let mut author_file_pcts: HashMap<usize, Vec<(String, f64)>> = HashMap::new();
    let mut author_files_owned: HashMap<usize, usize> = HashMap::new();

    for (path, blame_lines) in &snapshot.blame_map {
        let total = blame_lines.len().max(1);
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for bl in blame_lines {
            *counts.entry(bl.author_id).or_insert(0) += 1;
        }
        for (&author_id, &count) in &counts {
            *author_lines.entry(author_id).or_insert(0) += count;
            let pct = count as f64 / total as f64 * 100.0;
            author_file_pcts
                .entry(author_id)
                .or_default()
                .push((path.to_string_lossy().to_string(), pct));
            if pct > 50.0 {
                *author_files_owned.entry(author_id).or_insert(0) += 1;
            }
        }
    }

    let mut cards: Vec<AuthorCard> = snapshot
        .authors
        .iter()
        .map(|author| {
            let commit_ids = snapshot
                .commits_by_author
                .get(&author.id)
                .cloned()
                .unwrap_or_default();

            let author_commits: Vec<&crate::snapshot::Commit> = commit_ids
                .iter()
                .filter_map(|cid| snapshot.commits.iter().find(|c| &c.id == cid))
                .collect();

            let commit_count = author_commits.len();

            // Last active
            let last_active = author_commits
                .iter()
                .map(|c| c.timestamp)
                .max()
                .unwrap_or(snapshot.created_at);
            let days_since_active = (now - last_active).num_days().max(0);

            // Commit quality: score each message 0-100
            let avg_commit_quality = if author_commits.is_empty() {
                0.0
            } else {
                let total_q: f64 = author_commits
                    .iter()
                    .map(|c| score_commit_message(&c.message))
                    .sum();
                total_q / author_commits.len() as f64
            };

            // Top files by blame %
            let mut file_pcts = author_file_pcts
                .get(&author.id)
                .cloned()
                .unwrap_or_default();
            file_pcts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let top_files: Vec<String> = file_pcts
                .iter()
                .take(5)
                .map(|(path, _)| path.clone())
                .collect();

            // Directories touched (unique parent dirs from commits)
            let mut dirs = std::collections::HashSet::new();
            for commit in &author_commits {
                for fc in &commit.files_changed {
                    if let Some(parent) = fc.path.parent() {
                        dirs.insert(parent.to_string_lossy().to_string());
                    }
                }
            }

            AuthorCard {
                name: author.name.clone(),
                email: author.email.clone(),
                commit_count,
                files_owned: *author_files_owned.get(&author.id).unwrap_or(&0),
                lines_owned: *author_lines.get(&author.id).unwrap_or(&0),
                avg_commit_quality,
                top_files,
                last_active,
                days_since_active,
                directories_touched: dirs.len(),
            }
        })
        .collect();

    // Sort by commit count descending
    cards.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
    cards
}

/// Score a commit message from 0 to 100.
/// Rewards: length > 10 chars, conventional prefix (feat:/fix:/etc.), no "wip".
fn score_commit_message(msg: &str) -> f64 {
    let mut score = 0.0;
    let trimmed = msg.trim();
    let len = trimmed.len();

    // Length score: 0-40 points
    score += match len {
        0..=3 => 0.0,
        4..=10 => 10.0,
        11..=50 => 30.0,
        _ => 40.0,
    };

    // Conventional commit prefix: +30 points
    let prefixes = ["feat:", "fix:", "docs:", "style:", "refactor:", "perf:", "test:", "chore:", "ci:", "build:"];
    if prefixes.iter().any(|p| trimmed.starts_with(p)) {
        score += 30.0;
    }

    // Has a body or is descriptive (contains space after first word): +20 points
    if trimmed.contains('\n') || len > 20 {
        score += 20.0;
    }

    // Penalty for low-effort messages
    let lower = trimmed.to_lowercase();
    if lower == "wip" || lower == "fix" || lower == "update" || lower == "." {
        score = score.min(10.0);
    }

    // Cap at 100
    score += 10.0; // base points for having any message
    score.min(100.0)
}
```

- [ ] **Step 4: Wire `build_author_cards()` into `build_report()`**

In `src/scorer.rs` `build_report()` function (line 127-157), add the call and field:

```rust
pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
    weights: &[(&str, f64)],
) -> AnalysisReport {
    let overall_score = compute_overall_score_with_weights(&categories, weights);
    let top_actions = generate_top_actions(&categories);
    let file_hotspots = build_hotspots(snapshot);
    let coupling_pairs = build_coupling_pairs(snapshot);
    let author_ownership = build_author_ownership(snapshot);
    let file_ages = build_file_ages(snapshot);
    let author_cards = build_author_cards(snapshot);       // <-- ADD

    AnalysisReport {
        repo_name: snapshot.name.clone(),
        branch: snapshot.default_branch.clone(),
        time_window_months: snapshot.time_window.default_months,
        total_commits: snapshot.commits.len(),
        total_authors: snapshot.authors.len(),
        total_files: snapshot.files.len(),
        overall_score,
        categories,
        top_actions,
        remote_meta,
        file_hotspots,
        coupling_pairs,
        author_ownership,
        file_ages,
        author_cards,                                       // <-- ADD
        history: Vec::new(),
    }
}
```

- [ ] **Step 5: Run tests to verify everything passes**

Run: `cargo test --lib build_author_cards_from_snapshot && cargo test --lib build_report_has_author_cards_field`
Expected: both pass

- [ ] **Step 6: Write edge-case test — empty snapshot produces no cards**

```rust
#[test]
fn build_author_cards_empty_snapshot() {
    let snapshot = RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "t".into(),
        "main".into(),
        TimeWindow::default(),
    );
    let cards = build_author_cards(&snapshot);
    assert!(cards.is_empty());
}
```

Run: `cargo test --lib build_author_cards_empty_snapshot`
Expected: pass

- [ ] **Step 7: Write test — commit quality scoring**

```rust
#[test]
fn score_commit_message_quality() {
    // Conventional commit with description
    assert!(score_commit_message("feat: add login flow with validation") > 80.0);
    // Short but valid
    assert!(score_commit_message("fix: typo") > 40.0);
    // Wip — penalized
    assert!(score_commit_message("wip") < 20.0);
    // Empty
    assert!(score_commit_message("") < 15.0);
}
```

Run: `cargo test --lib score_commit_message_quality`
Expected: pass

---

## Chunk 2: HTML Tab — CSS + JS + Registration

### Task 3: Add CSS for author cards

**Files:**
- Modify: `src/renderer/html.rs` (CSS constant, around line 37)

- [ ] **Step 1: Add CSS rules to the `CSS` constant**

Append inside the `CSS` string (before the closing `"#`), after existing styles:

```css
.ac-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 16px;
  padding: 24px;
}
.ac-card {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 12px;
  padding: 20px;
  transition: border-color 0.15s;
}
.ac-card:hover {
  border-color: #334155;
}
.ac-name {
  font-size: 16px;
  font-weight: 700;
  color: #e2e8f0;
  margin-bottom: 2px;
}
.ac-email {
  font-size: 11px;
  color: #64748b;
  margin-bottom: 12px;
}
.ac-stats {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px 16px;
  margin-bottom: 12px;
}
.ac-stat-label {
  font-size: 11px;
  color: #64748b;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.ac-stat-value {
  font-size: 18px;
  font-weight: 700;
  color: #e2e8f0;
}
.ac-badge {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
}
.ac-files {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #1e293b;
}
.ac-file-item {
  font-size: 12px;
  color: #94a3b8;
  padding: 2px 0;
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ac-toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 16px 24px;
  border-bottom: 1px solid #1e293b;
  background: #0d1117;
  flex-wrap: wrap;
}
.ac-search {
  background: #161b22;
  border: 1px solid #1e293b;
  border-radius: 6px;
  color: #e2e8f0;
  padding: 6px 12px;
  font-size: 13px;
  min-width: 200px;
}
.ac-search:focus {
  outline: none;
  border-color: #3b82f6;
}
.ac-sort-btn {
  background: #161b22;
  border: 1px solid #1e293b;
  border-radius: 6px;
  color: #94a3b8;
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
}
.ac-sort-btn.active {
  border-color: #3b82f6;
  color: #e2e8f0;
}
```

- [ ] **Step 2: Verify CSS compiles**

Run: `cargo test --lib html_is_valid_document`
Expected: pass (CSS is inlined in the HTML output)

---

### Task 4: Add JS `buildAuthorsTab()` function

**Files:**
- Modify: `src/renderer/html.rs` (JS section in `build_js()`)

- [ ] **Step 1: Add `buildAuthorsTab()` function in the JS string**

Insert before `function renderApp()` (around line 2517):

```javascript
  function buildAuthorsTab() {
    var container = el('div');
    container.append(buildTabInfo(
      'Author Report Cards',
      'Per-contributor metrics derived from git blame and commit history. ' +
      'Files owned = files where author has >50% blame lines. ' +
      'Commit quality scores message length, conventional prefixes, and penalizes low-effort messages.',
      [
        { label: 'Active', detail: 'committed in last 30 days', color: '#10b981' },
        { label: 'Aging', detail: '30-90 days since last commit', color: '#f59e0b' },
        { label: 'Stale', detail: '90+ days since last commit', color: '#ef4444' }
      ]
    ));

    var cards = (R.author_cards || []).slice();
    if (cards.length === 0) {
      var empty = el('div', { style: { padding: '48px', textAlign: 'center', color: '#64748b' } });
      empty.append(txt('No author data available. Run with blame enabled.'));
      container.append(empty);
      return container;
    }

    // Toolbar
    var toolbar = el('div', { className: 'ac-toolbar' });
    var search = el('input', { className: 'ac-search', placeholder: 'Filter by name or email...' });
    search.setAttribute('type', 'text');

    var sortOptions = [
      { key: 'commits', label: 'Commits' },
      { key: 'files', label: 'Files Owned' },
      { key: 'active', label: 'Last Active' },
      { key: 'quality', label: 'Commit Quality' }
    ];
    var currentSort = 'commits';

    var sortBtns = sortOptions.map(function(opt) {
      var btn = el('button', { className: 'ac-sort-btn' + (opt.key === 'commits' ? ' active' : '') });
      btn.append(txt(opt.label));
      btn.addEventListener('click', function() {
        currentSort = opt.key;
        toolbar.querySelectorAll('.ac-sort-btn').forEach(function(b) { b.className = 'ac-sort-btn'; });
        btn.className = 'ac-sort-btn active';
        renderCards();
      });
      return btn;
    });

    toolbar.append(search);
    sortBtns.forEach(function(b) { toolbar.append(b); });

    var grid = el('div', { className: 'ac-grid' });

    function activityColor(days) {
      if (days <= 30) return '#10b981';
      if (days <= 90) return '#f59e0b';
      return '#ef4444';
    }

    function qualityColor(q) {
      if (q >= 70) return '#10b981';
      if (q >= 40) return '#f59e0b';
      return '#ef4444';
    }

    function renderCards() {
      var filtered = cards.filter(function(c) {
        var q = search.value.toLowerCase();
        if (!q) return true;
        return c.name.toLowerCase().indexOf(q) >= 0 ||
               c.email.toLowerCase().indexOf(q) >= 0;
      });

      filtered.sort(function(a, b) {
        if (currentSort === 'commits') return b.commit_count - a.commit_count;
        if (currentSort === 'files') return b.files_owned - a.files_owned;
        if (currentSort === 'active') return a.days_since_active - b.days_since_active;
        if (currentSort === 'quality') return b.avg_commit_quality - a.avg_commit_quality;
        return 0;
      });

      grid.replaceChildren();
      filtered.forEach(function(c) {
        var card = el('div', { className: 'ac-card' });

        // Header
        var nameEl = el('div', { className: 'ac-name' });
        var badge = el('span', { className: 'ac-badge' });
        badge.style.backgroundColor = activityColor(c.days_since_active);
        nameEl.append(badge, txt(c.name));
        var emailEl = el('div', { className: 'ac-email' });
        emailEl.append(txt(c.email));

        // Stats grid
        var stats = el('div', { className: 'ac-stats' });

        function addStat(label, value) {
          var lbl = el('div', { className: 'ac-stat-label' });
          lbl.append(txt(label));
          var val = el('div', { className: 'ac-stat-value' });
          val.append(txt(String(value)));
          stats.append(lbl, val);
        }

        addStat('Commits', c.commit_count);
        addStat('Files Owned', c.files_owned);
        addStat('Lines Owned', c.lines_owned);
        addStat('Dirs Touched', c.directories_touched);

        // Quality bar
        var qLabel = el('div', { className: 'ac-stat-label', style: { marginTop: '8px' } });
        qLabel.append(txt('Commit Quality'));
        var qBar = el('div', { style: {
          height: '6px', borderRadius: '3px', background: '#1e293b', marginTop: '4px'
        }});
        var qFill = el('div', { style: {
          height: '100%', borderRadius: '3px', width: Math.round(c.avg_commit_quality) + '%',
          background: qualityColor(c.avg_commit_quality)
        }});
        qBar.append(qFill);
        var qVal = el('div', { style: { fontSize: '11px', color: '#94a3b8', marginTop: '2px' } });
        qVal.append(txt(Math.round(c.avg_commit_quality) + '/100'));

        // Last active
        var activeEl = el('div', { style: { fontSize: '12px', color: activityColor(c.days_since_active), marginTop: '8px' } });
        var daysText = c.days_since_active === 0 ? 'Active today' :
                       c.days_since_active === 1 ? '1 day ago' :
                       c.days_since_active + ' days ago';
        activeEl.append(txt(daysText));

        // Top files
        var filesDiv = el('div', { className: 'ac-files' });
        if (c.top_files && c.top_files.length > 0) {
          var fLabel = el('div', { className: 'ac-stat-label' });
          fLabel.append(txt('Top Files'));
          filesDiv.append(fLabel);
          c.top_files.forEach(function(f) {
            var fi = el('div', { className: 'ac-file-item' });
            fi.append(txt(f));
            filesDiv.append(fi);
          });
        }

        card.append(nameEl, emailEl, stats, qLabel, qBar, qVal, activeEl, filesDiv);
        grid.append(card);
      });
    }

    search.addEventListener('input', renderCards);
    renderCards();

    container.append(toolbar, grid);
    return container;
  }
```

- [ ] **Step 2: Verify JS compiles into HTML**

Run: `cargo test --lib html_is_valid_document`
Expected: pass

---

### Task 5: Register Authors tab in `renderApp()` and add HTML tests

**Files:**
- Modify: `src/renderer/html.rs` (renderApp + tests)

- [ ] **Step 1: Register the tab in `renderApp()`**

In `src/renderer/html.rs`, update the tab arrays (line 2544-2553):

Change:
```javascript
    var tabNames = ['Overview', 'Hotspots', 'Coupling', 'Ownership', 'Age', 'Treemap', 'Trends'];
    var tabContents = [
      buildOverviewTab,
      buildHotspotsTab,
      buildCouplingTab,
      buildOwnershipTab,
      buildAgeTab,
      buildTreemapTab,
      buildTrendsTab
    ];
```

To:
```javascript
    var tabNames = ['Overview', 'Hotspots', 'Coupling', 'Ownership', 'Age', 'Treemap', 'Trends', 'Authors'];
    var tabContents = [
      buildOverviewTab,
      buildHotspotsTab,
      buildCouplingTab,
      buildOwnershipTab,
      buildAgeTab,
      buildTreemapTab,
      buildTrendsTab,
      buildAuthorsTab
    ];
```

- [ ] **Step 2: Update `make_report()` test helper (already done in Task 1 Step 4)**

Confirm `author_cards: vec![]` is present.

- [ ] **Step 3: Write test — Authors tab appears in HTML**

```rust
#[test]
fn html_contains_authors_tab() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("Authors"), "Should have Authors tab");
}
```

Run: `cargo test --lib html_contains_authors_tab`
Expected: pass

- [ ] **Step 4: Write test — Authors tab has card grid CSS class**

```rust
#[test]
fn html_authors_has_card_grid() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("ac-grid"), "Should have ac-grid CSS class for card layout");
}
```

Run: `cargo test --lib html_authors_has_card_grid`
Expected: pass

- [ ] **Step 5: Write test — Authors tab has search input**

```rust
#[test]
fn html_authors_has_search() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("ac-search"), "Should have ac-search input for filtering");
}
```

Run: `cargo test --lib html_authors_has_search`
Expected: pass

- [ ] **Step 6: Write test — Authors tab has sort buttons**

```rust
#[test]
fn html_authors_has_sort_buttons() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("ac-sort-btn"), "Should have sort button CSS class");
}
```

Run: `cargo test --lib html_authors_has_sort_buttons`
Expected: pass

- [ ] **Step 7: Write test — Authors tab has activity color logic**

```rust
#[test]
fn html_authors_has_activity_coloring() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("activityColor"),
        "Should have activityColor function for green/yellow/red status"
    );
}
```

Run: `cargo test --lib html_authors_has_activity_coloring`
Expected: pass

- [ ] **Step 8: Write test — Authors tab info banner**

```rust
#[test]
fn html_authors_has_info_banner() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Author Report Cards"),
        "Authors tab should have info banner with title"
    );
}
```

Run: `cargo test --lib html_authors_has_info_banner`
Expected: pass

- [ ] **Step 9: Run full test suite**

Run: `cargo test`
Expected: all tests pass, no regressions

---

## Verification Checklist

- [ ] `cargo test` — all tests pass
- [ ] `cargo clippy` — no warnings
- [ ] `cargo fmt --check` — formatted
- [ ] Generate a report on a real repo and visually confirm the Authors tab renders correctly with cards, sorting, filtering, and activity colors
