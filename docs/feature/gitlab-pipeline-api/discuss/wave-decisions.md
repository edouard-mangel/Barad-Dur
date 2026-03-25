# Wave Decisions: gitlab-pipeline-api

## Decision Log

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 1 | Feature Type | Infrastructure | CI/CD tooling, no user-facing UI changes |
| 2 | Walking Skeleton | Brownfield — mostly exists | Docker image, CI pipeline, JSON output, remote URL support all present |
| 3 | UX Research Depth | Lightweight | Happy path focus, minimal emotional arc; callers are CI pipelines, not humans |
| 4 | JTBD | Skipped | Infrastructure feature with clear technical motivation |
| 5 | Primary Persona | DevOps engineer / pipeline author | Person configuring the calling pipeline |
| 6 | Secondary Actor | CI pipeline (automated) | The actual runtime caller — no human in the loop |

## Brownfield Assessment

### Existing Assets (reusable as-is)

- **Docker image** (`Dockerfile`): scratch-based, ~31MB, includes git + barad-dur binary + SSL certs
- **Docker CI job** (`.gitlab-ci.yml` `docker` stage): builds and pushes to `CI_REGISTRY_IMAGE`
- **JSON output**: `barad-dur analyze <target> --json --pretty` writes structured JSON to stdout
- **Remote URL support**: `barad-dur analyze https://...` auto-clones to temp dir
- **Gate command**: `barad-dur gate . --min-score 70` exits non-zero on failure
- **Skip-blame**: `--skip-blame` for faster partial analysis
- **Cache**: `--no-cache` / `--cache-only` flags

### New Work Required

1. A new CI job (or child pipeline) that responds to pipeline triggers with variables
2. Documentation of the trigger API contract (variables, artifact paths)
3. Error handling for invalid inputs (bad URL, auth failure, timeout)
4. Example caller pipeline snippet

### Not Required (out of scope)

- No HTTP server or REST API — GitLab pipeline triggers are the API
- No changes to the Rust codebase — the CLI already does everything needed
- No new Docker image variant
