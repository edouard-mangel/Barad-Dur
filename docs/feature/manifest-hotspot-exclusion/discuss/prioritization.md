# Prioritization — manifest-hotspot-exclusion

Ranked by outcome impact vs. effort. Higher = do first.

| Rank | Item | Impact | Effort | Decision |
|------|------|--------|--------|----------|
| 1 | S1: exclude core manifests (US-1..3) | High — removes the daily noise that prompted the feature | Low — append globs + tests in one file | **Build now** |
| 2 | S2: opt-out granularity (US-4) | Low-Med — only matters if someone wants manifests back | Med — new config knob + tests + docs | **Defer** (DESIGN open question) |
| 3 | S3: mine manifest signal | Med — new insight, but speculative | High — multi-ecosystem parsing | **Out of scope** (separate idea) |

## Rationale

The feature's entire value sits in S1, which is also the cheapest. S2/S3 are
independent and unproven; pulling them in now would add config surface and parsing
complexity for demand that does not yet exist. Ship S1, observe, revisit.
