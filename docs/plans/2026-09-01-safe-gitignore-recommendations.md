# Safe gitignore recommendations implementation plan

**Ticket:** BD-001 — Prevent destructive gitignore false positives
**Priority:** P0
**Goal:** Barad-dûr must never recommend removing recognized source code from Git merely because its path contains words such as `secret` or `credentials`.

## Context

The Sovrium field test exposed a destructive false positive. `Gitignore coverage` scored 20/100 and ranked this action second:

> Add suspicious files to .gitignore and remove from tracking

The six findings were TypeScript source/test modules such as `root-secret.ts`, `auth-secret.ts`, `signing-secret.ts`, and `redact-secrets.ts`. Thirteen other files imported them. Following the recommendation would remove core cryptographic code and break the build.

The current implementation in `src/metrics/hygiene.rs` performs a lowercase substring search over the entire path. Therefore the pattern `secret` matches any source filename, regardless of extension, role, imports, or content. The generic Top Action in `src/scorer/actions.rs` then turns that low-confidence name match into destructive instructions.

## Design decision

Ship two independent safety barriers:

1. **Detection barrier:** semantic filename patterns such as `secret` and `credentials` do not classify recognized source-code extensions as gitignore candidates.
2. **Recommendation barrier:** the metric never tells users to remove files automatically. It asks them to review the listed paths and ignore/untrack only files confirmed to be generated, local, or credential material.

This first iteration will not inspect file contents or calculate entropy. `RepoSnapshot` does not retain general file contents, so content classification would expand collection/cache scope and still require a separately calibrated confidence model. Conservative path classification fixes the demonstrated defect with a small, auditable change.

## Intended behavior

| Path | Result | Reason |
|---|---|---|
| `.env` | Flag | Exact local-environment filename |
| `.env.production` | Flag | Environment-file family |
| `.env.example` | Do not flag | Intentional versioned template |
| `certs/server.pem` | Flag | Sensitive key/certificate extension |
| `node_modules/pkg/index.js` | Flag | Generated/dependency directory rule outranks source extension |
| `src/root-secret.ts` | Do not flag | `secret` is only semantic naming inside recognized source |
| `src/redact-secrets.test.ts` | Do not flag | Test source is still source code for this safety decision |
| `config/credentials.json` | Flag | Semantic filename in a non-source/config data file |
| `docs/secret-management.md` | Do not flag | Documentation about secrets is not credential material |
| `monkey.rs` | Do not flag | No substring-based `.key` accident |

Directory and exact/extension rules remain authoritative even when a file inside them has a source extension. For example, tracked `node_modules/index.js` must remain suspicious.

## Architecture

Replace the flat `SUSPICIOUS_PATTERNS` substring list with a pure path classifier in `src/metrics/hygiene.rs`:

```rust
fn suspicious_tracked_reason(path: &Path) -> Option<&'static str>
```

The classifier applies rules in this order:

1. Suspicious directory components (`node_modules`, `__pycache__`).
2. Exact filenames and filename families (`.env`, `.env.*`, `.DS_Store`, `Thumbs.db`) with explicit safe template exceptions.
3. Sensitive/generated extensions (`key`, `pem`, `p12`, `pfx`, `pyc`).
4. Semantic filename tokens (`secret`, `secrets`, `credential`, `credentials`) only when the path is neither recognized source code nor documentation.

Use component, filename, extension, and token comparisons—not arbitrary full-path substring matching. Reuse `crate::metrics::file_role::has_source_extension` and `classify` rather than introducing another source-extension list.

The metric can continue returning `RawValue::List(Vec<String>)`; no public schema migration or cache-version bump is required. Reasons can remain internal in this iteration unless implementation shows they materially improve the report.

## Task 1 — Characterize the regression with failing tests

**Files:**

- Modify tests in `src/metrics/hygiene.rs`

Add a small `FileEntry` test helper if one does not already exist locally, then add these tests before changing detection:

### 1.1 Source modules containing secret terminology are safe

Create a snapshot containing:

- `src/infrastructure/crypto/root-secret.ts`
- `src/cli/secret.ts`
- `src/application/redact-secrets.ts`
- `src/infrastructure/auth/auth-secret.ts`
- `src/application/signing-secret.ts`
- `src/application/redact-secrets.test.ts`

Assert:

- `Gitignore coverage` returns `RawValue::Count(0)`.
- The score is `Some(100)`.
- None of the Sovrium paths appears in evidence.

### 1.2 Source exemption does not hide generated/dependency directories

Add `node_modules/package/index.ts` and `src/__pycache__/generated.py`. Assert both remain findings even though they carry source-like extensions.

### 1.3 Exact sensitive-file rules remain active

Cover `.env`, `.env.production`, `certs/server.pem`, `private/signing.key`, `.DS_Store`, `Thumbs.db`, and `cache/value.pyc`.

### 1.4 Safe templates and documentation are not findings

Cover `.env.example`, `.env.sample`, `.env.template`, `docs/secret-management.md`, and `docs/credentials.md`.

### 1.5 Matching uses path boundaries

Cover `src/monkey.rs`, `src/secretary.ts`, `docs/credentials-overview.txt`, and a directory whose ordinary name merely contains one of the old substrings. None should be flagged.

Run:

```bash
cargo test metrics::hygiene::tests::gitignore
```

Expected before implementation: the Sovrium source test fails because the existing `secret` substring rule reports all six files.

## Task 2 — Introduce a conservative path classifier

**Files:**

- Modify `src/metrics/hygiene.rs`
- Reuse `src/metrics/file_role.rs`

### 2.1 Replace the flat substring constant

Define separate, intention-revealing rule groups:

- `SUSPICIOUS_DIRECTORY_NAMES`
- `SUSPICIOUS_EXACT_FILE_NAMES`
- `SUSPICIOUS_EXTENSIONS`
- `SAFE_ENV_TEMPLATE_NAMES`
- `SENSITIVE_NAME_TOKENS`

Keep comparisons ASCII-case-insensitive where existing behavior is case-insensitive.

### 2.2 Implement component-aware helpers

Add focused helpers rather than one compound closure:

```rust
fn has_suspicious_directory(path: &Path) -> bool
fn is_env_file(name: &str) -> bool
fn has_sensitive_name_token(path: &Path) -> bool
fn suspicious_tracked_reason(path: &Path) -> Option<&'static str>
```

Tokenize the filename stem on non-alphanumeric separators so `auth-secret` matches but `secretary` does not. Do not search parent directory names for semantic tokens; a source tree named `secrets/` may legitimately implement secret management.

### 2.3 Apply source/docs safety before semantic rules

For semantic names only:

```rust
if has_source_extension(path) || classify(path) == FileRole::Docs {
    return None;
}
```

Do this after exact directory/environment/extension rules so tracked dependency trees and private-key files still flag.

### 2.4 Simplify `gitignore_coverage`

Replace its inline matching closure with:

```rust
.filter(|file| suspicious_tracked_reason(&file.path).is_some())
```

Keep deterministic snapshot order unless tests show the collector does not guarantee it; if necessary, sort the evidence paths before returning them.

Run:

```bash
cargo test metrics::hygiene::tests::gitignore
cargo test metrics::hygiene::tests
```

## Task 3 — Make Top Actions non-destructive

**Files:**

- Modify `src/scorer/actions.rs`
- Modify tests in `src/scorer/actions.rs`

Change the action from:

> Add suspicious files to .gitignore and remove from tracking

to wording equivalent to:

> Review suspicious tracked files; ignore and untrack only confirmed credentials, local files, or generated artifacts

The exact final sentence should satisfy these constraints:

- Begins with review/verification, not mutation.
- Does not imply every listed file is unsafe.
- Makes untracking conditional on confirmation.
- Names the intended categories so the user knows what to verify.

Add a direct assertion for `suggest_action("Gitignore coverage")` that checks the full stable wording. Also assert it does not contain an unconditional phrase such as `remove from tracking`.

Run:

```bash
cargo test scorer::actions::tests::suggest_action
```

## Task 4 — Align user-facing explanations

**Files:**

- Modify `src/renderer/templates/chrome.js`
- Modify `README.md` only if it contains prescriptive removal language after implementation
- Modify `docs/adr/001-architecture-decisions.md` if its metric description still implies name-only matching

Update the Gitignore coverage tooltip to say that Barad-dûr identifies **high-confidence path shapes for review**, not that every match should be removed. Mention that recognized source files are not treated as credentials based on semantic filenames alone.

Do not rewrite old historical design documents unless they are presented as current behavior. Plans under `docs/plans/` can remain historical; the ADR and current README must match the shipped rule.

Add or update the HTML string-presence test if one exists for metric tooltips. Do not add a browser test solely for copy if the renderer already has template-content tests.

## Task 5 — Add end-to-end metric/action regression coverage

**Files:**

- Prefer existing report/action tests in `src/scorer.rs` or `src/scorer/actions.rs`
- Modify `src/renderer/html/tests.rs` only if needed to observe the full flow

Construct a report input where the only files with suspicious words are recognized source modules. Assert:

- Gitignore coverage scores 100.
- No Gitignore coverage Top Action is generated.

Construct a second input with a real `.env` finding. Assert:

- The metric is scored below 100.
- Its Top Action uses the conditional, non-destructive wording.
- Evidence contains `.env` and not an unrelated source file.

This test protects both safety barriers from drifting independently.

## Task 6 — Verification and dogfood

Run focused checks:

```bash
cargo test metrics::hygiene::tests::gitignore
cargo test scorer::actions::tests
```

Run the full project gate:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check
```

Dogfood against a minimal fixture or Sovrium checkout containing the six reported source paths plus one real `.env` file. Confirm:

- The six source modules disappear from Gitignore coverage findings.
- The genuine `.env` remains.
- The metric score and Top Actions reflect only genuine candidates.
- No output instructs the user to remove files without review.

## Non-goals

- Reading every suspicious file’s contents during collection.
- Secret scanning or credential-value validation.
- Entropy detection.
- Automatically editing `.gitignore` or running `git rm --cached`.
- Detecting secrets embedded inside otherwise ordinary source files.
- Redesigning `MetricValue` evidence into a structured confidence schema.

Those may become separate tickets if conservative path detection leaves meaningful false negatives. They are not required to eliminate the demonstrated destructive false positive.

## Risks and mitigations

### False negatives for source files that actually contain hard-coded secrets

This metric checks repository hygiene, not source secret values. Source secret scanning requires content-aware security tooling and should not be approximated from filenames. The safer failure mode is to miss a source file here than recommend deleting application code.

### `.env.example` policy differs between teams

Most teams intentionally track templates. Treat known template suffixes as safe by default; teams can use dedicated secret scanning if their policy differs.

### New source languages drift from the exemption

Reuse `has_source_extension` as the existing source-language authority. Add a safety-net test that iterates representative supported extensions with a `secret` stem. Future language additions should update that shared classifier once.

### Top Action wording drifts back to destructive language

Pin the complete action string and add a negative assertion against unconditional removal wording.

## Definition of done

- All six Sovrium source/test examples are regression-tested and no longer flagged.
- True-positive credential/generated path fixtures remain flagged.
- Semantic matches are token- and role-aware rather than full-path substrings.
- Top Actions requires human verification before ignore/untrack advice.
- Current documentation and tooltip describe conservative behavior.
- Focused tests, formatting, clippy, full tests, and `git diff --check` pass.
