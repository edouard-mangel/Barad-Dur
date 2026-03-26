# Journey Visualization -- Cross-Repository Coupling Detection

## Journey: Adriana detects coupling across 24 microservice repositories

### Emotional Arc

```
Confidence
    ^
    |                                              ***** EMPOWERED
    |                                         ****       "I can see exactly which
    |                                    ****             repos are too coupled"
    |                               ****
    |                          ****
    |                     ****  INFORMED
    |                ****       "Coupling data is emerging"
    |           ****
    |      ****  CURIOUS
    |  ****      "Let's see what the tool finds"
    | *
    |* UNCERTAIN
    | "I suspect coupling but have no proof"
    +-------------------------------------------------> Time
     Discover    Collect     Analyze     Review      Share
     repos       snapshots   coupling    results     findings
```

---

## Step 1: Discover Repositories

### Context
Adriana wants to scan all repos under `/home/adriana/work/services/` for cross-repo coupling. She runs the new `coupling` subcommand.

### ASCII Mockup -- CLI Invocation

```
$ barad-dur coupling /home/adriana/work/services/

  Scanning for git repositories...
  Found 24 repositories:
    payment-gateway      ./services/payment-gateway
    billing-service      ./services/billing-service
    notification-svc     ./services/notification-svc
    user-auth            ./services/user-auth
    api-gateway          ./services/api-gateway
    shared-libs          ./services/shared-libs
    ... (18 more)

  Analysis scope: 276 repo pairs | Time window: 6 months (default)
```

### ASCII Mockup -- With Options

```
$ barad-dur coupling /home/adriana/work/services/ --window 3months --min-coupling 30

  Scanning for git repositories...
  Found 24 repositories.
  Analysis scope: 276 repo pairs | Time window: 3 months | Threshold: 30%
```

### Emotional State
UNCERTAIN -> CURIOUS: "Found 24 repositories" confirms the tool understood the directory. Seeing "276 repo pairs" sets expectations for the scope.

---

## Step 2: Collect Snapshots

### Context
The tool collects git metadata (commit timestamps, authors, file lists) per repo. This reuses the existing `Collector -> Snapshot` pipeline from barad-dur.

### ASCII Mockup -- Collection Progress

```
  Collecting git history...

  [============>                       ]  10/24  notification-svc

  Completed:
    payment-gateway ........... 1,247 commits     0.3s (cached)
    billing-service ...........   892 commits     0.2s (cached)
    notification-svc ..........   634 commits     analyzing...
    user-auth .................   421 commits     0.3s (cached)
    shared-libs ...............   312 commits     28s

  Estimated time remaining: ~2 min
```

### ASCII Mockup -- Non-repo directory skipped

```
  [SKIP] legacy-monolith: not a git repository
  [SKIP] temp-experiment: no commits

  22/24 repositories collected. 2 skipped.
```

### Emotional State
CURIOUS -> INFORMED: Progress bar + per-repo commit counts build confidence. Skipped repos are reported clearly.

---

## Step 3: Analyze Coupling

### Context
Pairwise coupling analysis runs across all valid repo pairs. For temporal coupling: commit timestamps within the configured window are correlated. The tool shows progress across the 231 valid pairs (22 repos after 2 skipped).

### ASCII Mockup -- Coupling Analysis Progress

```
  Analyzing temporal coupling across 231 repo pairs...

  [==================>                 ]  142/231 pairs

  Strong coupling detected so far: 8 pairs above 30%
```

### Emotional State
INFORMED: The analysis is running. Early signals ("8 pairs above 30%") build anticipation.

---

## Step 4: Review Results -- CLI Output

### Context
Coupling analysis complete. Results displayed as a ranked list of coupling pairs.

### ASCII Mockup -- Temporal Coupling Results (R1)

```
  CROSS-REPOSITORY COUPLING REPORT
  =================================
  Repos scanned: 22 (2 skipped)  |  Time window: 6 months
  Generated: 2026-03-25 14:32 UTC

  TEMPORAL COUPLING (commits within 24h window)
  -----------------------------------------------
  #   Repo A               Repo B               Score   Co-changes  Confidence
  1.  payment-gateway      billing-service       78%     42/54       HIGH
  2.  catalog-service      search-indexer        65%     28/43       HIGH
  3.  notification-svc     user-auth             52%     19/37       MEDIUM
  4.  api-gateway          shared-libs           48%     15/31       MEDIUM
  5.  billing-service      notification-svc      41%     12/29       MEDIUM
  6.  payment-gateway      api-gateway           35%     11/31       MEDIUM
  7.  order-service        inventory-svc         33%     8/24        LOW
  8.  shared-libs          user-auth             31%     9/29        LOW

  Below threshold (< 30%): 223 pairs not shown. Use --all to display.

  SUMMARY
  -------
  High coupling (>= 60%): 2 pairs
  Medium coupling (30-59%): 6 pairs
  Total significant pairs: 8 of 231

  2 repositories skipped:
    legacy-monolith: not a git repository
    temp-experiment: no commits
```

### ASCII Mockup -- Multi-Dimension Results (R2)

```
  CROSS-REPOSITORY COUPLING REPORT
  =================================
  Repos scanned: 22 (2 skipped)  |  Time window: 6 months
  Generated: 2026-03-25 14:32 UTC

  COUPLING PAIRS (ranked by combined score)
  ------------------------------------------
  #   Repo A               Repo B               Temporal  Team   Deps   Combined
  1.  payment-gateway      billing-service       78%       45%    3 shared  HIGH
  2.  catalog-service      search-indexer        65%       80%    1 shared  HIGH
      ^ Team: Yuki Tanaka is the ONLY shared author (bus factor risk)
  3.  shared-libs          payment-gateway       48%       20%    5 shared  HIGH
      ^ Deps: payment-gateway imports shared-libs (Cargo.toml)
  4.  notification-svc     user-auth             52%       35%    0 shared  MEDIUM
  5.  api-gateway          shared-libs           48%       25%    4 shared  MEDIUM
  6.  billing-service      notification-svc      41%       15%    1 shared  MEDIUM

  TEAM COUPLING RISKS
  --------------------
  Single-author bridges (1 person connecting 2 repos):
    Yuki Tanaka: search-indexer <> catalog-service (42 + 18 commits)
    Tomasz Wierzbicki: shared-libs <> 5 repos (sole maintainer)

  DEPENDENCY COUPLING
  --------------------
  shared-libs has 5 direct consumers:
    payment-gateway, billing-service, api-gateway, user-auth, notification-svc
  Blast radius of shared-libs change: 5 repositories
```

### ASCII Mockup -- Verbose mode (-v)

```
  1.  payment-gateway <> billing-service
      Temporal: 78% (42 co-changes within 24h out of 54 payment-gw commits)
      Team:     45% (3 shared authors out of 7 unique across both)
               Shared: Adriana Kowalski (lead), Carlos Mendez, Lisa Park
      Deps:     3 shared (shared-libs v0.4.2, serde 1.0.195, tokio 1.35)
```

### Emotional State
EMPOWERED: Adriana can see the coupling landscape. payment-gateway and billing-service confirmed as the most coupled pair. Yuki's bridge role is visible. Shared-libs blast radius is quantified.

---

## Step 5: Share Findings

### ASCII Mockup -- JSON output (R2)

```
$ barad-dur coupling /home/adriana/work/services/ --json -o coupling-report.json

  Analyzing 22 repositories...
  [====================================]  231/231 pairs

  Coupling report written to coupling-report.json
```

### ASCII Mockup -- HTML output (R3)

```
$ barad-dur coupling /home/adriana/work/services/ --html --open

  Analyzing 22 repositories...
  [====================================]  231/231 pairs

  Opening coupling visualization in browser...
```

### Emotional State
EMPOWERED -> SATISFIED: Report is ready to share with the CTO.

---

## Error Path: No repos found

```
$ barad-dur coupling /tmp/empty/

  Scanning for git repositories...
  Found 0 repositories at /tmp/empty/

  Error: No git repositories found. Provide a directory containing git repos:
    barad-dur coupling /path/to/repos/
```

---

## Error Path: Only one repo found

```
$ barad-dur coupling /home/adriana/work/services/payment-gateway

  Scanning for git repositories...
  Found 1 repository. Cross-repo coupling requires 2+ repositories.

  For single-repo coupling analysis, use:
    barad-dur analyze /home/adriana/work/services/payment-gateway
```

---

## Error Path: Permission denied on some repos

```
  [SKIP] restricted-repo: permission denied
  [SKIP] archived-project: not a git repository

  20/22 repositories collected. 2 skipped.
  Proceeding with coupling analysis on 20 repos (190 pairs)...
```

---

## TUI Mockup -- HTML Coupling Visualization (R3 conceptual layout)

```
+------------------------------------------------------------------+
|  COUPLING: /home/adriana/work/services/     2026-03-25 14:32 UTC  |
|  22 repos | 8 significant pairs | Window: 6 months               |
+------------------------------------------------------------------+
|                                                                    |
|  [Graph]  [Matrix]  [Pairs]  [Teams]  [Dependencies]              |
|                                                                    |
|  GRAPH TAB:                                                        |
|                                                                    |
|     (payment-gw) ====== (billing-svc)                              |
|         |    \                                                     |
|         |     --- (api-gw)                                         |
|         |                                                          |
|    (shared-libs) --- (user-auth)                                   |
|         |                                                          |
|    (notification)                                                  |
|                                                                    |
|  Edge thickness = coupling strength                                |
|  [x] Temporal  [x] Team  [x] Dependencies                         |
|                                                                    |
|  MATRIX TAB:                                                       |
|  +----------+--------+--------+--------+--------+--------+         |
|  |          | pay-gw | bill   | notif  | user   | shared |         |
|  | pay-gw   |   --   |  78%   |  22%   |  18%   |  48%   |         |
|  | bill     |  78%   |   --   |  41%   |  15%   |  35%   |         |
|  | notif    |  22%   |  41%   |   --   |  52%   |  12%   |         |
|  | user     |  18%   |  15%   |  52%   |   --   |  31%   |         |
|  | shared   |  48%   |  35%   |  12%   |  31%   |   --   |         |
|  +----------+--------+--------+--------+--------+--------+         |
|  Color: red >= 60% | yellow 30-59% | green < 30%                   |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Shared Artifacts Identified in This Journey

| Artifact | Source | Consumers | Notes |
|----------|--------|-----------|-------|
| `${root_dir}` | CLI positional arg | Repo discovery | Root directory to scan for git repos |
| `${discovered_repos}` | Directory scan | Snapshot collection, pair analysis | List of valid git repo paths |
| `${skipped_repos}` | Discovery + collection | CLI output footer | Repos skipped with reason |
| `${repo_snapshots}` | Per-repo git log collection | Coupling analyzers | Commit timestamps, authors, file lists per repo |
| `${time_window}` | CLI `--window` or default (6 months) | Temporal coupling analyzer | Duration to look back in git history |
| `${coupling_window}` | CLI `--coupling-window` or default (24h) | Temporal coupling analyzer | Max time between commits to count as co-change |
| `${coupling_pairs}` | Pairwise coupling analysis | CLI renderer, JSON renderer, HTML renderer | Ranked list of repo pairs with scores per dimension |
| `${min_coupling}` | CLI `--min-coupling` or default (30%) | Output filtering | Threshold below which pairs are hidden |
| `${team_bridges}` | Author overlap analysis | CLI output, HTML teams tab | Single-author bridges flagged as bus factor risks |
| `${dependency_map}` | Manifest file scanning | CLI output, HTML deps tab | Directed dependency graph between repos |
