# Safety-Invariant Verification (DISCUSS) — manifest-hotspot-exclusion

Evidence for **NFR-1**: excluding manifests from the snapshot does not change the
deps/CVE category or dependency-based coupling outputs. Both consume manifests via
**disk reads independent of the snapshot**, verified by code inspection during DISCUSS.

## Deps / CVE category — reads lockfiles from disk

`src/collector/deps.rs`:
- `collect_locked_deps(repo_root: &Path)` (line 11) dispatches to per-ecosystem
  parsers, each of which joins onto `repo_root` and reads the file directly:
  - `parse_cargo_lock`: `repo_root.join("Cargo.lock")` → `std::fs::read_to_string` (lines 21-22)
  - `parse_npm_lock`: `repo_root.join("package-lock.json")` (line 47)
  - `parse_pip_lock`: `repo_root.join("requirements.txt")` (line 77)
  - `parse_nuget_lock`: `repo_root.join("packages.lock.json")` (line 107)
- No reference to `RepoSnapshot` / the snapshot file set anywhere in the module.

**Conclusion**: deps reads *lockfiles* (and `requirements.txt`) from disk. It does not
read `package.json` at all, and never consults the snapshot. Unaffected by snapshot
exclusion.

## Dependency-based coupling — reads manifests from disk

`src/coupling/dependency.rs`:
- Manifest read path: `repo_path.join("package.json")` then
  `std::fs::read_to_string` → `parse_package_json` (lines 301-304).
- `parse_package_json` (line 90) parses content passed in; sibling parsers exist for
  `Cargo.toml`, `go.mod`, `*.csproj` (lines 59, 110, 181).
- Reads are rooted at `repo_path`, not the snapshot file set.

**Conclusion**: dependency coupling reads manifests from disk. Unaffected by snapshot
exclusion.

## Application point of exclusion (for completeness)

`src/collector/snapshot_builder.rs:~96` applies `is_excluded(...)` once while building
the snapshot's `files`. Excluded paths are dropped *only* from the snapshot — the disk
remains untouched, so the disk-read consumers above see no change.

## Net invariant
Adding manifest globs to `DEFAULT_EXCLUDE_PATTERNS` (`src/collector/exclude.rs`)
removes manifests from snapshot-derived surfaces (hotspots, complexity, churn,
snapshot-based coupling) while leaving deps/CVE and dependency-coupling outputs
byte-for-byte identical. AC-5 pins this with a regression test.
