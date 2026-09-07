# Long Methods calibration (BD-006)

BD-006 replaces the legacy `LOC > 40 OR CC > 10` predicate with:

```text
CC > 10
OR
(CC > 5 AND LOC > applicable LOC threshold)
```

The applicable LOC threshold is 80 for lowercase `.tsx` and `.jsx` files and
40 for every other supported source file. Counts below use the same committed
date boundary and pinned commit for both predicates.

| Repository | Language | Legacy | Tiered | Removed |
|---|---:|---:|---:|---:|
| barad-dur | Rust | 159 | 55 | 104 |
| ripgrep | Rust | 113 | 62 | 51 |
| helix | Rust | 365 | 176 | 189 |
| starship | Rust | 171 | 62 | 109 |
| dotnet-starter-kit | C# | 74 | 54 | 20 |
| evolutionary-architecture-by-example | C# | 2 | 0 | 2 |
| eShopModernizing | C# | 569 | 564 | 5 |
| App-Serveat | C# | 51 | 26 | 25 |
| payp-app-front | TypeScript | 87 | 28 | 59 |
| kairis-crm | TypeScript | 132 | 117 | 15 |
| mautic | PHP | 2,026 | 1,677 | 349 |

The captured Sovrium report contains 386 findings among 3,482 functions. Of
those findings, 245 have CC <= 5 and are necessarily removed by the complexity
floor, leaving at most 141. All 10 findings with CC >= 15 remain because they
exceed the unconditional complexity threshold. The captured aggregate does not
contain enough per-function data to determine how many additional CC 6-10
`.tsx`/`.jsx` findings are removed by the higher UI LOC threshold.

## Decision-surface review

Five of the eleven field-test baselines change:

| Repository | Long Methods | Health | Overall | Action effect |
|---|---:|---:|---:|---|
| barad-dur | 50 -> 75 | 42 -> 47 | 70 -> 72 | none |
| helix | 50 -> 75 | 51 -> 56 | 62 -> 64 | none |
| starship | 50 -> 75 | 56 -> 61 | 67 -> 68 | none |
| evolutionary-architecture-by-example | 75 -> 100 | 93 -> 100 | 97 -> 100 | Long Methods action removed |
| payp-app-front | 25 -> 50 | 68 -> 75 | 83 -> 86 | same action, updated score |

The other six repositories retain their existing score bands and decision
surfaces. No coupling counts, unrelated categories, or unrelated actions
change.

The Overall column was restated when this branch was rebased onto the
unscored-category change (`HISTORY_SCHEMA_VERSION` 4), which renormalises the
overall across measurable categories only and so moved the *starting* overall
on the three repositories with an unscored category: barad-dur 73 -> 70,
evolutionary-architecture-by-example 98 -> 97, payp-app-front 88 -> 83. Long
Methods and Health, the columns this calibration is actually about, are
unchanged by that rebase — the recalibration's own effect is identical before
and after.
