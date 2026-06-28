# Story Map — manifest-hotspot-exclusion

## Backbone (user activities)

```
Run analysis ──────▶ Review hotspots ──────▶ Trust the ranking
```

## Walking skeleton (minimum end-to-end value)

> Add core-ecosystem manifest globs to `DEFAULT_EXCLUDE_PATTERNS`, with unit tests in
> `exclude.rs` proving manifests are excluded under defaults and retained when
> defaults are off.

This single slice delivers the whole outcome — the pipeline downstream of
`is_excluded()` already exists and needs no change. There is no smaller slice that
delivers value, and no larger one required for v1.

## Release slices

| Slice | Stories | Outcome | In this feature? |
|-------|---------|---------|------------------|
| S1 — Exclude core manifests | US-1, US-2, US-3 | Manifests gone from hotspots; deps/coupling intact | ✅ Yes |
| S2 — Opt-out granularity | US-4 (open) | Re-include manifests without losing other defaults | ⏳ Deferred → DESIGN open question |
| S3 — Mine manifest signal | — | Measure dependency volatility / version-range risk | ❌ Separate Ideas.md entry |

## Dependency notes

- S1 has no dependency on S2 or S3.
- S2 only becomes necessary if a user reports wanting manifests back without losing
  lockfile/generated-dir exclusion. YAGNI until then.
