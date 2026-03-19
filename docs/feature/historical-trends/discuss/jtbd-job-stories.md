# JTBD Job Stories — historical-trends

## Context

barad-dur produces a single-point-in-time snapshot of repository health. Users currently have no way to know whether a score of 62 is improving or declining. Historical trend analysis answers: "Is this repo getting better or worse, and how fast?"

No prior interview data exists. This analysis derives jobs from domain observation: users of repo-analysis tools, common pain patterns in engineering health dashboards, and the forces visible in the feature request itself.

---

## Persona: Marco Rossi

**Who**: Engineering lead at a 12-person product company. Runs barad-dur monthly to check on team health. Started after a near-miss: a bus-factor crisis revealed when a key developer quit.

**Demographics**:
- Experienced developer (8 years), now mostly in code review and planning
- Runs barad-dur on 3-6 repos; reads CLI output on terminal, occasionally opens HTML report
- Primary concern: catching quality decay before it becomes an incident
- Frequency: monthly ritual; also on-demand after merging large features

**Pain Point**: Marco gets a score of 58 today. He does not know if that's a recovery (was 45 last month) or a slide (was 65). He cannot tell his team "we're improving" or "we have a problem" — he can only say "we're at 58."

---

## Persona: Priya Nair

**Who**: Senior developer on a platform team that owns a legacy Rust codebase. She is the de-facto quality keeper — runs analysis tools, files tech-debt tickets, champions refactoring.

**Demographics**:
- Runs barad-dur after every major refactoring sprint to validate impact
- Shares reports in team retrospectives; needs data to back up investment requests
- Audience is skeptical: manager asks "is refactoring actually helping?"
- Frequency: every 2-4 weeks; also used in sprint reviews

**Pain Point**: Priya spent 2 weeks extracting a 3,000-line God module. She re-ran barad-dur and the score improved from 51 to 68. But she has no artifact showing the before/after — just her word. She needs a trend chart to show the sprint review.

---

## Job Stories

### JS-01: Track direction of health over time

**When** I have been running analyses for several weeks and I read today's score,
**I want to** see whether my score has gone up or down compared to previous runs,
**so I can** tell my team confidently whether our quality investments are paying off.

#### Three Job Dimensions
- **Functional**: Compare current score to historical scores across multiple past analyses
- **Emotional**: Feel confident making claims about trajectory, not just state
- **Social**: Be seen as a data-driven lead who can prove outcomes, not just assert them

#### Forces
- **Push**: Current single-point score gives no directional signal; Marco has had to keep manual spreadsheets
- **Pull**: A trend line in the CLI output would immediately answer "better or worse?"
- **Anxiety**: "Will re-analysis at past commits take hours and lock up my terminal?"
- **Habit**: Running `barad-dur analyze .` once and reading the score — no history subcommand in muscle memory yet

---

### JS-02: Validate the impact of a refactoring effort

**When** I finish a major refactoring sprint and re-run analysis,
**I want to** see a before/after comparison of scores and category breakdowns,
**so I can** demonstrate to my manager that the refactoring delivered measurable quality improvement.

#### Three Job Dimensions
- **Functional**: Compare scores at two specific points in time (before and after refactoring)
- **Emotional**: Feel vindicated — turn "I believe this helped" into "the data shows this helped"
- **Social**: Justify investment in tech debt to stakeholders who are skeptical of quality work

#### Forces
- **Push**: "I have no artifact — just my word" is career risk when justifying refactoring budget
- **Pull**: Before/after table in JSON/HTML would be shareable in Confluence or Slack
- **Anxiety**: "What if the before-commit is deep in history and analysis fails silently?"
- **Habit**: Running `barad-dur analyze .` only on the current HEAD

---

### JS-03: Catch slow-burn quality decay before it becomes an incident

**When** I monitor a repository over many weeks without seeing obvious code changes,
**I want to** see a trend that alerts me when scores are declining steadily across multiple periods,
**so I can** intervene proactively before a team emergency (bus factor crisis, build instability, mounting tech debt).

#### Three Job Dimensions
- **Functional**: Visualize score trends over an extended window (months); detect negative direction
- **Emotional**: Feel in control — not perpetually anxious about hidden decay
- **Social**: Be proactive rather than reactive; be the person who spotted it before it was a crisis

#### Forces
- **Push**: Marco's near-miss: the bus-factor crisis was only discovered when someone quit. It had been worsening for 4 months
- **Pull**: A trend view would surface "Bus Factor score: -3 points/month for 4 months" before the human signal arrives
- **Anxiety**: "Running analysis for each of 20 past commits will be extremely slow — I might not have time to wait"
- **Habit**: Point-in-time check; Marco rarely looks at more than the last run

---

### JS-04: Integrate trend data into CI/CD pipelines

**When** I set up automated analysis in CI/CD,
**I want to** record analysis results over time and surface trend data in JSON format,
**so I can** build dashboards, write alerting scripts, or diff scores between releases automatically.

#### Three Job Dimensions
- **Functional**: Write trend data to a machine-readable format; read previous run data programmatically
- **Emotional**: Feel like the tool fits professional workflows, not just ad-hoc use
- **Social**: Present the team as using sophisticated, automated quality tracking

#### Forces
- **Push**: `--json` output today only gives current state; piping it to a dashboard means losing history
- **Pull**: A trend JSON schema would integrate with existing monitoring tooling (Grafana, custom scripts)
- **Anxiety**: "If the trend storage format changes between versions, my scripts will break"
- **Habit**: Using `barad-dur analyze . --json -o report.json` and parsing it; no trend-aware workflow yet

---

## Job Map: Track Repo Health Over Time

Walking the 8 universal steps for JS-01 (primary job):

| Step | User Goal | Desired Outcome |
|------|-----------|-----------------|
| 1. Define | Know what time range to inspect and which repos matter | Minimize time to configure meaningful trend window |
| 2. Locate | Find or generate past analysis data for comparison | Minimize likelihood of missing historical snapshots |
| 3. Prepare | Configure how often/when trend snapshots are recorded | Minimize effort to set up ongoing trend tracking |
| 4. Confirm | Verify historical data is available and fresh | Minimize likelihood of comparing against stale or wrong commits |
| 5. Execute | Run analysis and retrieve trend view | Minimize time from command invocation to trend output |
| 6. Monitor | Read trend line; see direction and velocity | Minimize time to understand whether trajectory is positive or negative |
| 7. Modify | Drill into specific time period or category | Minimize effort to understand what caused a score change |
| 8. Conclude | Export or share the trend result | Minimize friction in sharing trend evidence with stakeholders |

---

## Four Forces Summary Table

| Job Story | Push | Pull | Anxiety | Habit |
|-----------|------|------|---------|-------|
| JS-01 | Manual spreadsheet tracking; no directional signal | Trend line answers "better or worse?" instantly | Re-analysis hours-long; terminal blocks | Single `analyze .` invocation, no history |
| JS-02 | No artifact to justify refactoring investment | Before/after table shareable in reviews | Past commit analysis may fail silently | Only ever analyzes current HEAD |
| JS-03 | Bus-factor crisis discovered after the fact | Proactive decay alert before human signal | 20-commit analysis prohibitively slow | Point-in-time checks only |
| JS-04 | `--json` loses history between runs | Trend JSON enables Grafana/dashboards | Format changes break downstream scripts | `analyze . --json -o report.json` |

---

## Primary Job (for story prioritization)

**JS-01** is the primary job. It is triggered by every user type, has the strongest push force, and unlocks JS-02, JS-03, and JS-04 as enrichments. The walking skeleton must deliver JS-01.

JS-02 and JS-03 are enrichments on top of the same data model.
JS-04 is an integration job that shapes the schema/format contract.
