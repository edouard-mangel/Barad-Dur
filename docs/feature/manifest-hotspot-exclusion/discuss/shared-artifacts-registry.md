# Shared Artifacts Registry — manifest-hotspot-exclusion

Every `${variable}` referenced across journey/requirements has one documented source.

| Artifact | Source of truth | Consumers |
|----------|-----------------|-----------|
| `${default_exclude_patterns}` | `src/collector/exclude.rs` → `DEFAULT_EXCLUDE_PATTERNS` | `is_excluded()` |
| `${manifest_globs}` | NEW entries appended to `${default_exclude_patterns}` | `is_excluded()` |
| `${snapshot}` | `src/collector/snapshot_builder.rs` (exclusion applied at line ~96) | all metrics, scorer, renderers |
| `${hotspot_list}` | `src/scorer/` (`HotspotFile`) derived from `${snapshot}` | CLI / HTML / dashboard renderers |
| `${use_defaults}` | config `exclude.use_defaults` (`src/config/mod.rs`); CLI `--no-default-excludes` | `is_excluded()` |
| `${deps_output}` | `src/collector/deps.rs` (reads lockfiles from disk) | deps category, registry/OSV |
| `${coupling_deps}` | `src/coupling/dependency.rs` (reads manifests from disk) | dependency-based coupling |

## Invariant note

`${deps_output}` and `${coupling_deps}` derive from **disk reads**, not `${snapshot}`.
Therefore changes to `${default_exclude_patterns}` cannot alter them. This is the
load-bearing fact that makes a snapshot-level manifest exclusion safe.
