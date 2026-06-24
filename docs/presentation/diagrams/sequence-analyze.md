# Sequence Diagram — `barad-dur analyze .`

Step-by-step walkthrough of what happens when a developer runs
`barad-dur analyze .` against a local repository. The diagram covers
the happy path with a warm cache; deviations (cache miss, remote URL,
`--deps` flag) are noted as `opt` blocks.

```mermaid
sequenceDiagram
    autonumber
    actor Dev as Developer
    participant CLI as CLI (clap)
    participant Cfg as Config Loader
    participant Runner as Runner
    participant Cache as Snapshot Cache<br/>(snapshot.bin)
    participant Collector as Collector<br/>(git2 + gitcli)
    participant Snapshot as RepoSnapshot
    participant Metrics as Metrics Engine<br/>(5 categories)
    participant Registry as Registry Client<br/>(opt: --deps)
    participant Scorer as Scorer
    participant Trend as Trend / History<br/>(trends.json)
    participant Renderer as Renderer

    Dev->>CLI: barad-dur analyze .
    CLI->>Cfg: load .repository-analysis/barad-dur.toml
    Cfg-->>CLI: RepoConfig (weights, thresholds, excludes)
    CLI->>Runner: resolve_snapshot(CollectOptions)

    Runner->>Cache: load snapshot.bin — is HEAD stale?

    alt Cache hit (HEAD unchanged)
        Cache-->>Runner: RepoSnapshot (deserialized from bincode)
    else Cache miss or --no-cache
        Runner->>Collector: open(path, time_window)
        Collector->>Collector: git2::Repository::discover(path)
        Collector-->>Runner: Collector handle

        Runner->>Collector: collect_commits() via libgit2
        Collector-->>Runner: Vec<Commit> + Vec<Author>

        Runner->>Collector: collect_files() — HEAD tree via libgit2
        Collector-->>Runner: Vec<FileEntry> (with blob OIDs)

        Runner->>Collector: collect_blame_cached() — parallel git blame
        Note over Collector: rayon parallel: one<br/>'git blame --porcelain'<br/>per file; blob-OID cache<br/>skips unchanged files
        Collector-->>Runner: HashMap<Path, Vec<BlameLine>>

        Runner->>Collector: collect_file_metrics() — tree-sitter AST
        Collector-->>Runner: HashMap<Path, FileComplexity>

        Runner->>Snapshot: build RepoSnapshot + build_indexes()
        Note over Snapshot: Derives: commits_by_file,<br/>commits_by_author,<br/>file_change_pairs,<br/>import_graph

        Runner->>Cache: save snapshot.bin (bincode)
    end

    Runner-->>CLI: RepoSnapshot

    CLI->>Metrics: compute_health(snapshot, thresholds)
    CLI->>Metrics: compute_team(snapshot, thresholds)
    CLI->>Metrics: compute_evolution(snapshot, thresholds)
    CLI->>Metrics: compute_hygiene(snapshot, thresholds)
    CLI->>Metrics: compute_coupling(snapshot, thresholds)
    Metrics-->>CLI: Vec<CategoryResult> (each scored 0–100)

    opt --deps flag
        CLI->>Registry: collect_locked_deps(path)
        Registry->>Registry: parse Cargo.lock / package-lock.json / requirements.txt
        Registry->>Registry: partition into cached / uncached deps
        Registry-->>Registry: fetch uncached: crates.io / npm / PyPI / NuGet
        Registry-->>Registry: fetch CVEs: OSV database
        Registry->>Cache: save deps-cache.json (7-day TTL)
        Registry-->>CLI: Vec<EcosystemReport>
        CLI->>Metrics: compute_deps(ecosystem_reports)
        Metrics-->>CLI: CategoryResult (Deps)
    end

    CLI->>Scorer: build_report(snapshot, categories, weights)
    Note over Scorer: Computes weighted overall score,<br/>top actions, hotspot files,<br/>coupling pairs, author cards,<br/>import cycles, file ages
    Scorer-->>CLI: AnalysisReport

    CLI->>Trend: load_history_checked(path)
    Trend-->>CLI: Vec<HistoryEntry> (prior runs)
    CLI->>Trend: compute_trend(history, branch, current_entry)
    Trend-->>CLI: TrendSummary (delta, velocity)
    CLI->>Trend: append_if_new_head(entry, path)

    CLI->>Renderer: render(report, format)
    Note over Renderer: Format selected by flags:<br/>default → CLI table<br/>--json → serde_json<br/>--html → self-contained HTML

    Renderer-->>Dev: Output to stdout / file / browser
```
