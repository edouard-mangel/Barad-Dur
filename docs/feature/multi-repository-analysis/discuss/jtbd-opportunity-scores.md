# JTBD Opportunity Scores -- Cross-Repository Coupling Detection

## Scoring Methodology

Using the ODI (Outcome-Driven Innovation) opportunity scoring framework:

**Opportunity Score = Importance + max(Importance - Satisfaction, 0)**

- Importance: How important is this outcome to the user? (1-10)
- Satisfaction: How well does the current solution satisfy this need? (1-10)
- Score range: 1-20 (>15 = overserved opportunity is rare; >12 = strong opportunity)

Scores are estimated from domain observation, persona analysis, and forces analysis. No direct user interviews were conducted.

---

## Job Step Opportunities

### JS-01: Detect Temporal Coupling Between Repos

| # | Desired Outcome | Importance | Satisfaction | Opportunity |
|---|----------------|------------|--------------|-------------|
| 1.1 | Minimize time to identify which repo pairs have correlated commit activity | 9 | 1 | 17 |
| 1.2 | Minimize false positives from coincidental commit timing | 8 | 1 | 15 |
| 1.3 | Minimize effort to configure time window for correlation | 6 | 2 | 10 |
| 1.4 | Minimize time to scan all repos in a root directory | 7 | 1 | 13 |
| 1.5 | Minimize effort to understand coupling strength (confidence) | 8 | 1 | 15 |
| 1.6 | Minimize missed coupling due to wrong analysis parameters | 7 | 1 | 13 |

**Top opportunities for JS-01**: 1.1 (identification speed), 1.2/1.5 (accuracy and confidence)

---

### JS-02: Detect Team Coupling (Shared Authors)

| # | Desired Outcome | Importance | Satisfaction | Opportunity |
|---|----------------|------------|--------------|-------------|
| 2.1 | Minimize time to identify which repos share the same committers | 8 | 1 | 15 |
| 2.2 | Minimize risk of missed author matches due to email variations | 7 | 1 | 13 |
| 2.3 | Minimize effort to spot single-author bridges (bus factor risk) | 9 | 1 | 17 |
| 2.4 | Minimize effort to see which specific people cross repo boundaries | 7 | 2 | 12 |

**Top opportunities for JS-02**: 2.3 (single-author bridges), 2.1 (shared committer identification)

---

### JS-03: Detect Dependency Coupling (Shared Libraries/APIs)

| # | Desired Outcome | Importance | Satisfaction | Opportunity |
|---|----------------|------------|--------------|-------------|
| 3.1 | Minimize time to map which repos depend on the same libraries | 9 | 2 | 16 |
| 3.2 | Minimize surprise when updating a shared library (blast radius) | 9 | 1 | 17 |
| 3.3 | Minimize effort to detect dependency direction (A depends on B vs mutual) | 7 | 1 | 13 |
| 3.4 | Minimize missed dependencies beyond lockfiles (internal APIs, protos) | 6 | 1 | 11 |

**Top opportunities for JS-03**: 3.2 (blast radius surprise), 3.1 (dependency mapping speed)

---

### JS-04: Visualize the Coupling Landscape

| # | Desired Outcome | Importance | Satisfaction | Opportunity |
|---|----------------|------------|--------------|-------------|
| 4.1 | Minimize time to produce a shareable coupling visualization | 8 | 1 | 15 |
| 4.2 | Minimize risk of unreadable graph at 20+ repos | 7 | 1 | 13 |
| 4.3 | Minimize effort to filter visualization by coupling dimension | 6 | 1 | 11 |
| 4.4 | Minimize effort to drill down from overview to specific pair details | 7 | 1 | 13 |

**Top opportunities for JS-04**: 4.1 (visualization speed), 4.2/4.4 (readability and drilldown)

---

## Opportunity Landscape

Plotting all outcomes by opportunity score:

```
Score  Outcome
 17    1.1  Minimize time to identify correlated commit activity across repos
 17    2.3  Minimize effort to spot single-author bridges (bus factor risk)
 17    3.2  Minimize surprise when updating a shared library (blast radius)
 16    3.1  Minimize time to map which repos depend on same libraries
 15    1.2  Minimize false positives from coincidental commit timing
 15    1.5  Minimize effort to understand coupling strength (confidence)
 15    2.1  Minimize time to identify repos sharing same committers
 15    4.1  Minimize time to produce shareable coupling visualization
 13    1.4  Minimize time to scan all repos in a root directory
 13    1.6  Minimize missed coupling due to wrong parameters
 13    2.2  Minimize risk of missed author matches (email variations)
 13    3.3  Minimize effort to detect dependency direction
 13    4.2  Minimize risk of unreadable graph at 20+ repos
 13    4.4  Minimize effort to drill down to specific pair details
 12    2.4  Minimize effort to see which people cross repo boundaries
 11    3.4  Minimize missed dependencies beyond lockfiles
 11    4.3  Minimize effort to filter by coupling dimension
 10    1.3  Minimize effort to configure time window
```

---

## Strategic Prioritization

### Tier 1: Must address (score >= 15) -- Release 1

These outcomes define the core value proposition:

- **Temporal coupling identification** (1.1): Core feature -- correlate commits across repos within time windows
- **Coupling strength and confidence** (1.2, 1.5): Show co-change count alongside percentage to distinguish signal from noise
- **Single-author bridge detection** (2.3): High-value, low-effort addition to temporal coupling data
- **Shared committer identification** (2.1): Author overlap is computed from the same git data as temporal coupling
- **Blast radius mapping** (3.2, 3.1): Dependency scanning via manifest files

Note: Tier 1 spans R1 (temporal) and R2 (team + dependency). R1 addresses 1.1, 1.2, 1.5 and the CLI output. R2 adds 2.1, 2.3, 3.1, 3.2.

### Tier 2: Should address (score 12-14) -- Release 2 + Release 3

- **Repo scanning speed** (1.4): Efficient directory traversal and snapshot building
- **Author email normalization** (2.2): Reliable matching across email configs
- **Dependency direction** (3.3): Show A-depends-on-B vs mutual dependency
- **Coupling visualization** (4.1): HTML graph + matrix for architecture reviews
- **Graph readability** (4.2): Threshold filtering to prevent hairball graphs
- **Drilldown capability** (4.4): Click a pair to see coupling details

### Tier 3: Nice to have (score < 12) -- Deferred

- **Person-level detail** (2.4): Show specific shared authors per pair
- **Beyond-lockfile detection** (3.4): Internal APIs, proto files, config schemas
- **Dimension filtering** (4.3): Toggle temporal/team/dependency in visualization
- **Time window configuration** (1.3): Sensible default covers most cases

---

## Opportunity-to-Story Mapping

| Opportunity | Story Candidate | Release |
|-------------|----------------|---------|
| 1.1, 1.2, 1.5 | Temporal coupling detection across repos with ranked CLI output | R1 |
| 1.4, 1.6 | Repo discovery and snapshot collection from root directory | R1 |
| 2.1, 2.3 | Team coupling: shared authors and single-author bridges | R2 |
| 3.1, 3.2, 3.3 | Dependency coupling: manifest scanning with blast radius | R2 |
| 2.2 | Author normalization for reliable matching | R2 |
| 4.1, 4.2, 4.4 | HTML visualization: interactive graph + coupling matrix | R3 |
| 4.3 | Dimension filtering in HTML | R3 |
