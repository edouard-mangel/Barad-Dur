# JTBD Job Stories -- Cross-Repository Coupling Detection

## Context

barad-dur detects temporal coupling between files WITHIN a single repository (`src/metrics/health.rs` uses `file_change_pairs` from `RepoSnapshot`). However, coupling between REPOSITORIES is invisible. Teams discover cross-repo coupling the hard way: "every time I change repo A, repo B breaks in CI." This feature extends coupling detection across repository boundaries.

The user's need is clear: "I need to detect wrong coupling between different repositories, so I can detect which repositories are too coupled." Input is a local directory containing multiple git repos. Output is both CLI (ranked coupling pairs) and HTML (interactive graph and matrix).

---

## Persona: Adriana Kowalski

**Who**: VP of Engineering at a 60-person SaaS company with 24 microservices across 4 teams. Noticed that changes to `payment-gateway` frequently break `billing-service` and `notification-svc`. Currently detects coupling manually by observing CI failure patterns.

**Demographics**:
- Non-daily CLI user; delegates most tooling to team leads
- Needs evidence-based data to justify decoupling investments to the CTO
- Primary concern: "which repos are too tightly coupled and where should we invest in decoupling?"
- Frequency: monthly for architecture reviews; ad-hoc when CI breakage patterns emerge

**Pain Point**: Adriana suspects 5-6 repo pairs are too coupled but cannot prove it. She pieces together coupling evidence from CI failure logs, Slack conversations, and Jira tickets. It takes days to build an incomplete picture. She wants one command that maps the coupling landscape across all 24 repos.

---

## Persona: Tomasz Wierzbicki

**Who**: Platform team lead. Owns shared libraries consumed by 12 services. Suspects `shared-libs` is a coupling bottleneck creating ripple effects but cannot quantify the blast radius.

**Demographics**:
- Heavy CLI user; automates everything through CI/CD
- Consumes JSON output; builds internal dashboards with Grafana
- Primary concern: "which repos depend on my shared libraries and how tightly?"
- Frequency: weekly automated runs; ad-hoc before major refactors

**Pain Point**: Tomasz updates `shared-libs` and 5 downstream services break. He has no map of dependency coupling. He manually checks `Cargo.toml`, `package.json`, and import paths across repos to estimate the blast radius. He wants automated dependency coupling detection.

---

## Persona: Yuki Tanaka

**Who**: Senior developer who joined 3 weeks ago. Assigned to `search-indexer` but keeps getting pulled into `catalog-service` changes because she is the only person who recently touched both.

**Demographics**:
- New to the codebase; uses tooling to build mental models
- Interested in understanding the repo relationship landscape
- Primary concern: "why do I keep getting pulled across repo boundaries?"
- Frequency: intensive use during first 2-4 weeks, then occasional

**Pain Point**: Yuki does not know which repos are connected and why. She is a knowledge bridge between `search-indexer` and `catalog-service` but nobody told her this explicitly. She wants a visualization showing repo relationships so she can understand the coupling landscape.

---

## Job Stories

### JS-01: Detect Temporal Coupling Between Repos

**When** I notice that CI breaks in `billing-service` often follow commits in `payment-gateway`,
**I want to** see which repository pairs have correlated commit activity within configurable time windows,
**so I can** identify hidden temporal coupling and plan targeted decoupling work.

#### Three Job Dimensions
- **Functional**: Surface repo pairs whose commits cluster together in time (within hours/days); rank by correlation strength
- **Emotional**: Move from "gut feeling that repos are coupled" to "data confirms coupling between payment-gateway and billing-service at 78%"
- **Social**: Present evidence to CTO and team leads to justify refactoring budgets

#### Forces
- **Push**: CI breakages across repos happen weekly; manual correlation is slow and unreliable
- **Pull**: A single command producing ranked temporal coupling pairs replaces weeks of manual investigation
- **Anxiety**: "What if the time window is wrong and the tool misses real coupling or flags noise?"
- **Habit**: Observing CI logs + Slack messages + Jira tickets to piece together coupling patterns manually

---

### JS-02: Detect Team Coupling (Shared Authors Across Repos)

**When** Yuki keeps getting pulled from `search-indexer` into `catalog-service` because she is the only person who knows both,
**I want to** see which repository pairs share the same committers,
**so I can** identify knowledge bottlenecks and bus factor risks that span repo boundaries.

#### Three Job Dimensions
- **Functional**: Identify author overlap between repo pairs; rank by shared author percentage; highlight single-author bridges
- **Emotional**: Move from "I'm the only one who can fix this" to "we have visibility into who crosses repo boundaries"
- **Social**: Team leads can redistribute work or hire to reduce single-person dependencies

#### Forces
- **Push**: Yuki is a single point of failure bridging two repos; nobody realized this until she got sick
- **Pull**: A team coupling report would show all cross-repo author overlaps at a glance
- **Anxiety**: "Will the author matching work across different git email configurations?"
- **Habit**: Asking around in Slack who knows which repo; tribal knowledge

---

### JS-03: Detect Dependency Coupling (Shared Libraries/APIs)

**When** Tomasz updates `shared-libs` and 5 downstream services break,
**I want to** see which repos depend on the same libraries, APIs, or contracts,
**so I can** map the blast radius of changes and plan interface stabilization.

#### Three Job Dimensions
- **Functional**: Detect shared dependencies across repo pairs (Cargo.toml, package.json, import paths, API contracts); show dependency direction
- **Emotional**: Move from "I have no idea what will break" to "I know exactly which 5 repos will be affected by this shared-libs change"
- **Social**: Platform team can communicate impact zones before making breaking changes

#### Forces
- **Push**: Tomasz broke 5 services last month with a shared-libs update; no blast radius map existed
- **Pull**: Automated dependency coupling detection with direction (A depends on B, both depend on C)
- **Anxiety**: "Can the tool detect dependencies beyond lockfiles -- internal APIs, shared proto files, config schemas?"
- **Habit**: Manually grepping Cargo.toml files across repos; asking teams in Slack which services use shared-libs

---

### JS-04: Visualize the Coupling Landscape

**When** Adriana prepares her quarterly architecture review and needs to communicate the coupling picture,
**I want to** generate an interactive visualization showing repo relationships as a graph and coupling matrix,
**so I can** communicate the coupling landscape to the CTO and team leads without explaining terminal output.

#### Three Job Dimensions
- **Functional**: Produce HTML with interactive graph (nodes=repos, edges=coupling) and matrix (rows/cols=repos, cells=scores); filterable by dimension
- **Emotional**: Move from "I spend hours assembling architecture diagrams" to "I generate a shareable coupling report in 2 minutes"
- **Social**: CTO and non-technical stakeholders can see which repos are dangerously coupled at a glance

#### Forces
- **Push**: Architecture review slides are manually assembled from scattered data; always outdated by presentation time
- **Pull**: Single command producing an interactive HTML coupling visualization
- **Anxiety**: "Will the graph be readable with 24 repos or will it be a hairball?"
- **Habit**: Drawing coupling diagrams in Miro or Lucidchart from memory and CI failure patterns

---

## Job Map: Detect Cross-Repository Coupling

Walking the 8 universal steps for JS-01 (primary job):

| Step | User Goal | Desired Outcome |
|------|-----------|-----------------|
| 1. Define | Choose root directory containing repos | Minimize effort to specify which repos to scan for coupling |
| 2. Locate | Find git repos in subdirectories | Minimize missed repos or false positives (non-repo dirs) |
| 3. Prepare | Collect commit data per repo for correlation | Minimize time to gather git history across all repos |
| 4. Confirm | Show discovered repos and analysis scope | Minimize surprise about which repos will be compared |
| 5. Execute | Correlate commit timestamps across repo pairs | Minimize wall-clock time for pairwise coupling computation |
| 6. Monitor | Display progress (N repo pairs analyzed) | Minimize uncertainty about how long the analysis will take |
| 7. Resolve | Rank pairs by coupling score per dimension | Minimize effort to identify the most coupled repo pairs |
| 8. Conclude | Output ranked pairs (CLI) or coupling visualization (HTML) | Minimize friction in communicating coupling findings |

---

## Four Forces Summary Table

| Job Story | Push | Pull | Anxiety | Habit |
|-----------|------|------|---------|-------|
| JS-01 Temporal | CI breakages across repos; manual correlation slow | Ranked temporal coupling pairs from one command | "Wrong time window = noise?" | Observing CI logs + Slack |
| JS-02 Team | Yuki is single point of failure bridging repos | Team coupling report showing all cross-repo overlaps | "Author email matching across configs?" | Asking in Slack who knows what |
| JS-03 Dependency | 5 services broke from shared-libs update | Automated blast radius map with direction | "Can it detect internal APIs, not just lockfiles?" | Manual grep of Cargo.toml across repos |
| JS-04 Visualization | Architecture slides assembled manually, always stale | Interactive HTML graph + matrix from one command | "Will 24-repo graph be readable?" | Miro/Lucidchart diagrams from memory |

---

## Primary Job (for story prioritization)

**JS-01** (temporal coupling) is the primary job. It has the strongest push force (CI breakage is the most painful and frequent signal), requires the least new infrastructure (git log data is already collected by barad-dur's existing snapshot), and validates the core coupling detection concept. Once temporal coupling works, team coupling (JS-02) and dependency coupling (JS-03) add dimensions, and visualization (JS-04) adds presentation quality.
