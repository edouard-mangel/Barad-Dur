# Journey: barad-dur backfill — Visual Map

## Persona
**Marco Rossi** (Engineering Lead) or **Priya Nair** (Senior Developer)
running `barad-dur backfill` on a repo that has never been analyzed before.

## Emotional Arc
Start: **Curious but uncertain** (will 3 years of history even show something useful?) →
Middle: **Engaged and watching** (progress ticking forward, each commit analyzed) →
End: **Confident and ready** (trend dashboard populated, can share results)

---

## Happy Path — Standard Backfill

```
[Trigger]                    [Sample]                   [Analyze each]
User runs                    Tool scans full             Git commands
barad-dur backfill           git log history            target SHAs
         |                          |                          |
         v                          v                          v
+--------+--------+    +-----------+-----------+   +----------+---------+
| $ barad-dur     |    | Collecting commit      |   | [1/10] Analyzing   |
|   backfill      |    | history...             |   |  commit abc1234... |
|                 |    |                        |   | [2/10] Analyzing   |
|                 |    | Found 847 commits.     |   |  commit def5678... |
|                 |    | Sampling 10 evenly     |   | ...                |
|                 |    | spaced commits.        |   | [10/10] Analyzing  |
+-----------------+    +------------------------+   |  commit xyz9999... |
                                                    +--------------------+
Feels: Curious                Feels: Informed            Feels: Engaged

         |
         v
[Store results]                                    [Verify + View]
Write each HistoryEntry                            Run analyze --trend
to .repository-analysis/trends.json               to see populated dashboard
         |                                                  |
         v                                                  v
+--------+--------------+                    +-------------+----------+
| Backfill complete.    |                    | $ barad-dur analyze .  |
|                       |                    |   --trend              |
| 10 entries written to |                    |                        |
| .repository-analysis/ |                    | Overall: 74  (+6 vs    |
| trends.json           |                    |  last)                 |
|                       |                    |                        |
| Run analyze --trend   |                    | Trend: 60→63→67→70→74  |
| to view dashboard.    |                    | Velocity: +1.2/wk      |
+-----------------------+                    +------------------------+

Feels: Satisfied                                   Feels: Confident
```

---

## Decision Branch — `--no-blame` Flag

```
Default (blame enabled)           --no-blame flag
         |                                |
         v                                v
Each commit analyzed              Each commit analyzed
with git blame calls              skipping blame computation
(accurate ownership data)         (faster: avoids slow git blame)
         |                                |
         v                                v
~15-30 min for 1000-commit        < 2 min for 1000-commit repo
repo (user's responsibility)      (performance target met)
         |                                |
         +----------+  +------------------+
                    |  |
                    v  v
           Same HistoryEntry schema
           written to trends.json
           (entries are identical
            except ownership
            data may be absent)
```

---

## Decision Branch — trends.json Already Has Entries

```
trends.json has 3 existing entries
(from prior regular analyze runs)
         |
         v
backfill reads existing SHA list
from trends.json
         |
         v
Sampling produces 10 commits
         |
         v
For each sampled commit:
  SHA already in trends.json? → SKIP (deduplication)
  SHA not in trends.json?     → ANALYZE and WRITE
         |
         v
[2/8] Skipping abc1234 (already analyzed)
[3/8] Analyzing def5678...
         |
         v
Result: only new SHAs written
Existing entries untouched
```

---

## Decision Branch — Already Backfilled (No-op Guard)

```
trends.json already has 10 entries
covering the sampled commits
         |
         v
All sampled SHAs found in trends.json
         |
         v
+------------------------------------------+
| Backfill already complete.               |
| 10 entries found in trends.json covering |
| the full commit range.                   |
| Nothing to do.                           |
+------------------------------------------+
Exit code: 0
```

---

## Error Path — Repo with 0 Commits

```
$ barad-dur backfill (on empty repo)
         |
         v
Tool calls: git log --format="%H" HEAD
         |
         v
git returns error or empty output
         |
         v
+-----------------------------------------------+
| Error: No commits found in this repository.   |
|                                               |
| barad-dur backfill requires at least one      |
| commit to analyze. Initialize your repository |
| with an initial commit and try again.         |
+-----------------------------------------------+
Exit code: 1
```

---

## Error Path — Git Command Fails Mid-Run

```
[5/10] Analyzing commit abc1234...
         |
         v
git ls-tree abc1234 fails
(e.g., object not found, permissions issue)
         |
         v
+-----------------------------------------------+
| Warning: Could not analyze commit abc1234.    |
| Skipping (git error: <message>).              |
| Continuing with remaining commits...          |
+-----------------------------------------------+
         |
         v
[6/10] Analyzing commit def5678...
         |
         v
Backfill completes with 9/10 entries written.
(skipped commits logged as warnings, not fatal)
```

---

## Small Repo Path — Fewer than 10 Commits

```
Repo has 5 commits total
         |
         v
+----------------------------------------+
| Found 5 commits.                       |
| Analyzing all commits (repo has fewer  |
| than 10 — no sampling needed).         |
+----------------------------------------+
         |
         v
[1/5] Analyzing commit abc1234...
...
[5/5] Analyzing commit xyz0000...
         |
         v
5 entries written to trends.json
```

---

## Integration Checkpoint

After backfill completes, the user should be able to run:

```
$ barad-dur analyze . --trend
```

and see the full trend table populated from the backfill entries. The dashboard
at `.repository-analysis/trends.json` uses the same `HistoryEntry` schema as
regular analyze runs. Backfill entries are indistinguishable from regular entries.
