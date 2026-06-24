# C4 Container Diagram — Barad-dûr

Internal components of Barad-dûr and how they relate to each other.
The binary is a single Rust crate; the containers below represent logical
modules with clear input/output contracts rather than separate deployed
services. The React Dashboard is a separate static web application bundled
under `dashboard/`.

```mermaid
C4Container
    title Container Diagram — Barad-dûr Internal Components

    Person(developer, "Developer / CI Pipeline")

    System_Boundary(baradur, "Barad-dûr (Rust binary)") {

        Container(cli, "CLI Layer", "Rust / clap", "Parses subcommands and flags: analyze, gate, backfill, init, coupling, watch, contributors. Dispatches to cmd/ handlers.")

        Container(remote, "Remote Analyzer", "Rust", "Detects GitHub/GitLab URLs, clones to a temp directory via git2, optionally enriches with GitHub API metadata (stars, issues, language).")

        Container(collector, "Collector", "Rust / git2 + rayon", "Reads git commits and file tree via libgit2. Runs parallel 'git blame --porcelain' (rayon). Computes tree-sitter AST complexity for 8 languages. Produces raw data for the snapshot.")

        Container(cache, "Snapshot Cache", "bincode on disk", "Serializes/deserializes RepoSnapshot to .repository-analysis/snapshot.bin. Invalidated when HEAD changes. Separate blame cache keyed by blob OID. History stored as NDJSON in trends.json.")

        Container(snapshot, "RepoSnapshot", "Rust struct", "Shared data model: commits, authors, file tree, blame map, complexity, import graph, file-change pairs. Build indexes (commits_by_file, commits_by_author, file_change_pairs) once on construction.")

        Container(metrics, "Metrics Engine", "Rust — pure functions", "Five category modules — Health, Team, Evolution, Git Hygiene, Coupling — plus optional Deps. Each is a pure function (snapshot) -> CategoryResult scoring 0–100.")

        Container(scorer, "Scorer", "Rust", "Aggregates CategoryResults into AnalysisReport: weighted overall score, top action suggestions, hotspot files, coupling pairs, author ownership, import cycles, file ages, author cards.")

        Container(renderer, "Renderer", "Rust", "Three output modes dispatched from cmd/analyze.rs: CLI (colored terminal table), JSON (serde_json), HTML (embedded JS/CSS via include_str!, single self-contained file).")

        Container(registry, "Registry Client", "Rust / ureq", "Queries crates.io, npm, PyPI, NuGet for package publish dates. Calls OSV database for CVE advisories. Results cached 7 days in .repository-analysis/deps-cache.json.")

        Container(config, "Config", "Rust / toml", "Loads .repository-analysis/barad-dur.toml. Merges with CLI flags. Provides category weights, exclude patterns, thresholds, output format, and component depth.")
    }

    Container(dashboard, "React Dashboard", "React 19 + Vite + Tailwind 4", "Browser-based drag-and-drop report viewer. Reads report.json dropped by the user. Renders all tabs: Overview, Health, Coupling, Team, Evolution, Hygiene, Deps, Trends.")

    Rel(developer, cli, "Invokes subcommand", "CLI")
    Rel(cli, remote, "Delegates when target is a URL")
    Rel(cli, config, "Loads config, merges flags")
    Rel(cli, collector, "Opens repo and triggers collection")
    Rel(remote, collector, "Passes cloned local path")
    Rel(collector, cache, "Reads/writes snapshot.bin and blame cache")
    Rel(collector, snapshot, "Builds RepoSnapshot")
    Rel(cache, snapshot, "Returns cached RepoSnapshot on hit")
    Rel(snapshot, metrics, "Passed as read-only input to all metric functions")
    Rel(metrics, scorer, "Returns Vec<CategoryResult>")
    Rel(scorer, renderer, "Returns AnalysisReport")
    Rel(renderer, developer, "Writes CLI output / JSON / HTML to stdout or file")
    Rel(cli, registry, "Triggers dep analysis when --deps flag is set")
    Rel(registry, metrics, "Returns EcosystemReport for Deps category")
    Rel(renderer, dashboard, "Exports report.json consumed by dashboard")
    Rel(developer, dashboard, "Opens in browser, drops report.json")

    UpdateLayoutConfig($c4ShapeInRow="3", $c4BoundaryInRow="1")
```
