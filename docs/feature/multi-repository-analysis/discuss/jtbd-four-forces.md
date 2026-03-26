# Four Forces Analysis -- Cross-Repository Coupling Detection

## Context

This analysis examines the forces acting on each persona when considering adoption of cross-repository coupling detection. The goal is to identify design implications that reduce anxiety and habit forces while amplifying push and pull.

---

## JS-01: Detect Temporal Coupling Between Repos

### Forces Diagram

```
        PUSH (toward switching)                    PULL (toward new solution)
        =============================              =============================
        CI breaks in billing-service               `barad-dur coupling ./repos/`
        follow commits in payment-gw.              produces ranked pairs:
        Adriana spends days piecing                "payment-gateway <> billing-svc
        together coupling from CI logs,            temporal: 78% (42 co-changes
        Slack, and Jira. Incomplete.               within 24h over 6 months)."
                                \                    /
                                 \                  /
                                  v                v
                             [DECISION TO SWITCH]
                                  ^                ^
                                 /                  \
                                /                    \
        ANXIETY (against switching)                HABIT (status quo inertia)
        =============================              =============================
        "What if the time window is                Observing CI failure patterns
        wrong? Too tight = misses real             in GitLab. Asking in Slack:
        coupling. Too loose = noise."              "Did anyone change payment-gw
        "24 repos = 276 pairs. Will                recently?" Tribal knowledge.
        the output be overwhelming?"

```

### Switch Likelihood: HIGH

Push is very strong (CI breakage is weekly, painful, and visible). Pull is strong (a single command replaces days of manual investigation). Anxiety is moderate and specific (time window tuning, output readability). Habit is moderate (manual correlation is painful but teams have adapted).

### Design Implications

1. **Configurable time window with sensible default**: Default to 24-hour window for temporal correlation. Allow `--window 48h` or `--window 7d` for different granularities. Document what the window means.
2. **Output must handle N^2 pairs gracefully**: 24 repos = 276 possible pairs. Show only pairs above a significance threshold. Sort by coupling score descending.
3. **Per-dimension filtering**: Allow `--temporal` to show only temporal coupling, reducing noise.
4. **Confidence indicator**: Show sample size (number of co-changes) alongside percentage to distinguish high-confidence from low-confidence scores.

---

## JS-02: Detect Team Coupling (Shared Authors)

### Forces Diagram

```
        PUSH                                       PULL
        =============================              =============================
        Yuki is the only person who                Team coupling report shows:
        bridges search-indexer and                  "search-indexer <> catalog-svc
        catalog-service. Nobody knew               team: 1 shared author (Yuki)
        until she got sick and both                 out of 5 total. Bus factor
        repos stalled.                             risk: HIGH."

                             [DECISION TO SWITCH]

        ANXIETY                                    HABIT
        =============================              =============================
        "Git email configs are messy.              Asking around: "Who knows the
        Will author matching work                  catalog-service?" Checking
        across repos with different                git blame manually. Tribal
        commit email addresses?"                   knowledge about who knows what.
```

### Switch Likelihood: HIGH (if author matching works reliably)

Push is strong (single-person bridges are invisible until they become incidents). Pull is strong (automated detection replaces tribal knowledge). Anxiety is specific and addressable (author normalization). Habit is low-moderate (no systematic tooling exists).

### Design Implications

1. **Author normalization**: Match authors by name, not just email. Group `yuki.tanaka@acme.com` and `yuki@personal.dev` as the same person if the name matches. Use existing `Author` struct from `RepoSnapshot`.
2. **Highlight single-author bridges**: When one person is the only shared author between two repos, flag it as a bus factor risk.
3. **Show shared authors by name**: Output lists specific people, not just percentages. "Shared: Yuki Tanaka (42 commits in search-indexer, 18 in catalog-service)."

---

## JS-03: Detect Dependency Coupling (Shared Libraries/APIs)

### Forces Diagram

```
        PUSH                                       PULL
        =============================              =============================
        Tomasz updated shared-libs                 Dependency coupling report:
        and 5 services broke. He had               "shared-libs -> payment-gw,
        no blast radius map. Manually              billing-svc, notification-svc,
        grepped Cargo.toml across 12               user-auth, api-gateway (5
        repos to find who depends on               consumers). Blast radius: 5."
        shared-libs.

                             [DECISION TO SWITCH]

        ANXIETY                                    HABIT
        =============================              =============================
        "Can it detect internal APIs               `grep -r 'shared-libs'` across
        and proto files, not just                  repo directories. Checking
        lockfiles? Our coupling is                 Cargo.toml and package.json
        in shared proto schemas."                  manually. Spreadsheet of deps.
```

### Switch Likelihood: MODERATE (depends on detection depth)

Push is strong (blast radius surprises are expensive). Pull is moderate (automated detection helps but may not cover all dependency types). Anxiety is HIGH on detection depth: users want more than lockfile scanning. Habit is moderate (manual grep + spreadsheets work but are incomplete).

### Design Implications

1. **Start with lockfile/manifest scanning**: Cargo.toml, package.json, go.mod, requirements.txt. These are reliable and structured.
2. **Pattern-based detection as extension**: Allow users to specify custom patterns (e.g., `--dependency-pattern "proto/*.proto"`) for internal APIs.
3. **Show dependency direction**: "A depends on B" is different from "A and B both depend on C." Direction matters for blast radius.
4. **Do not overcommit**: R2 scope. Start with manifest scanning; deep API analysis is future work.

---

## JS-04: Visualize the Coupling Landscape

### Forces Diagram

```
        PUSH                                       PULL
        =============================              =============================
        Architecture review slides                 `barad-dur coupling ./repos/
        are manually assembled from                --html --open` produces an
        CI logs, Miro diagrams, and                interactive graph: nodes are
        Slack history. Always outdated             repos, edges show coupling.
        by presentation time.                      Matrix view shows all pairs.

                             [DECISION TO SWITCH]

        ANXIETY                                    HABIT
        =============================              =============================
        "24 repos in a graph will be               Miro/Lucidchart diagrams drawn
        a hairball. Will it be                     from memory. Google Slides for
        readable?"                                 the CTO. Manual and tedious
        "Will the HTML look polished               but the format is familiar.
        enough for the CTO?"
```

### Switch Likelihood: MODERATE (depends on graph readability)

Push is moderate (quarterly effort, painful but infrequent). Pull is strong (automated visualization). Anxiety is HIGH on readability: force-directed graphs with 24+ nodes can be unreadable. Habit is moderate (manual diagrams are familiar to stakeholders).

### Design Implications

1. **Threshold filtering in graph**: Only show edges above a coupling threshold. A graph with 24 nodes but 8 significant edges is readable. A graph with 276 edges is not.
2. **Matrix as primary, graph as supplementary**: The coupling matrix (repos x repos, cells = scores) is always readable regardless of repo count. The graph is a visual supplement.
3. **Color coding by dimension**: Temporal = blue, team = orange, dependency = green. Combined score determines edge thickness.
4. **Filterable by dimension**: Toggle temporal/team/dependency edges independently in the HTML.

---

## Cross-Job Anxiety Themes

Three anxieties appear across multiple jobs and must be addressed in design:

| Anxiety | Jobs | Mitigation |
|---------|------|------------|
| Detection accuracy (false positives/negatives) | JS-01, JS-03 | Configurable thresholds + confidence indicators (sample size) |
| Output readability at scale (24+ repos) | JS-01, JS-04 | Significance threshold + matrix view + edge filtering |
| Author matching reliability | JS-02 | Name-based normalization + explicit author mapping config |

---

## Adoption Strategy

Based on forces analysis, the adoption path should be:

1. **JS-01 adopted first** (strongest push, least infrastructure): `barad-dur coupling /path/to/repos/` with temporal coupling CLI output
2. **JS-02 adopted second** (leverages same git data): Team coupling added to the same command
3. **JS-03 adopted third** (requires manifest scanning, new code): Dependency coupling as a new dimension
4. **JS-04 adopted last** (needs all dimensions for full value): HTML visualization of all coupling dimensions

This sequence informs release prioritization: temporal CLI first, team+dependency second, HTML visualization third.
