# Rust toolchain and dependency refresh — verification

Branch: `chore/rust-dependency-refresh`; base: `9c4a273` (`origin/main`).
Release/index checks started September 11, 2026; verification completed September 12.

## Scope

Pin Rust 1.98.1 for local development, both CI configurations, Docker, and the
release script. Refresh direct crate requirements and Cargo.lock. The lockfile
updates 95 existing crate names, adds 8, and removes 22. Node dependencies and
the Rust edition are unchanged.

Notable resolved upgrades:

| Crate | Before | After |
|---|---|---|
| colored | 2.2.0 | 3.1.1 |
| reqwest | 0.12.28 | 0.13.5 |
| toml | 0.8.23 | 1.1.6+spec-1.1.0 |
| tree-sitter | 0.26.9 | 0.27.0 |
| ignore | 0.4.27 | 0.4.33 |
| clap | 4.6.1 | 4.6.6 |

All direct dependencies were checked against the official crates.io sparse
index. Bincode is the explicit exception: 3.0.0 is published but its entire
`src/lib.rs` is `compile_error!("https://xkcd.com/2347/");`. Keep the usable
2.0.1 release and the existing cache format. The existing advisory exception
in deny.toml remains unchanged; this refresh does not resolve bincode's
maintenance status.

## P0 — release and API probes

- The [official release announcement](https://blog.rust-lang.org/releases/1.98.1/)
  and `https://static.rust-lang.org/dist/channel-rust-stable.toml` identify
  1.98.1, released 2026-09-03. Installed tool output:
  `rustc 1.98.1 (48a229cea 2026-09-01)` and
  `cargo 1.98.1 (797e8a9bc 2026-08-05)`.
- `docker manifest inspect rust:1.98.1` and
  `docker manifest inspect rust:1.98.1-alpine` both exited 0 and include
  `linux/amd64`. These are manifest checks, not cross-platform build tests.
- The first updated build emitted six E0616 errors: `QueryMatch::captures`
  is private. Tree-sitter 0.27's source exposes
  `pub const fn captures(&self) -> &[QueryCapture<'tree>]`, returning the same
  slice. All six callers now use it.
- Tree-sitter's `child_count()` returns `u32`; `named_child_count()` still
  returns `usize`. Remove the eleven redundant casts only from ordinary-child
  iteration. Retain the seven casts used for named-child iteration.
- After compilation, eight unit tests and four coupling integration tests
  failed. TOML 1.x's `Value::from_str` uses `ValueDeserializer` for a single
  value; it no longer parses a whole document. One observed error was
  `unexpected content, expected nothing`. Migrate all six affected production
  and test sites to `toml::from_str`; the existing assertions then pass.
- `cargo info reqwest@0.13.5` shows `default-tls = [rustls]`. Explicitly
  select `native-tls-no-alpn`, charset, HTTP/2, system proxy, blocking, and JSON
  to preserve the 0.12 feature choices. `cargo tree -e features -i reqwest`
  confirms that feature set, with no reqwest rustls feature enabled.

## P1 — invariant sweep

| Rule | Sites inspected | Evidence |
|---|---|---|
| Development/build/release compiler is 1.98.1 | rust-toolchain.toml; 10 GitLab image references; 4 GitHub toolchain inputs; Dockerfile; release.sh | All pins agree; no active 1.94 pin remains |
| Query captures retain their previous iteration semantics | counters.rs (5); treesitter.rs (1) | Accessor returns the same slice; parser tests and corpus pass |
| Child indices keep their required type | inheritance.rs (2 ordinary-child loops); pressman.rs (9); named-child loops in calls.rs, inheritance.rs, pressman.rs, rust_calls.rs | Only the eleven now-redundant casts removed; Clippy passes |
| TOML inputs remain whole documents | collector/deps.rs; config/mod.rs; coupling/dependency.rs; three init.rs test sites; existing config/field-test deserializers | Six migrated calls; existing document deserializers already use from_str; no Value::from_str document caller remains |
| TLS backend stays native | Cargo.toml; registry/client.rs; remote/github.rs | Resolved reqwest feature tree retains native TLS and prior optional capabilities |
| Report/cache/scoring contracts remain unchanged | generated report contract; serializer fixture; cache functions; all corpus surfaces | Contract checks pass; bincode remains 2.0.1; 11 corpus baselines match without edits |

The sweep covers the enumerated rules; it is not proof that every possible
dependency behavior change has been exercised.

## Verification results

- Baseline `cargo test --locked --all-features --no-fail-fast`:
  1,635 passed, 7 ignored, 35 suites. Initially the sandbox blocked the local
  socket timeout test; rerunning with socket access passed.
- Updated `RUSTFLAGS='-D warnings' cargo test --locked --all-features --no-fail-fast`:
  1,635 passed, 0 failed, 7 ignored, 35 suites.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: no issues.
  A new lint required removing one redundant reference in a test panic message;
  the affected `analyze_stdout_purity` integration test also passed afterward.
- `cargo fmt -- --check` and `git diff --check`: passed.
- `cargo deny check`: advisories, bans, licenses, and sources passed under the
  existing policy. Duplicate-version warnings remain for base64,
  core-foundation, and syn.
- `make report-contract-check`: generated declarations and producer fixture match.
- `pnpm -C dashboard check`: 38 tests passed; production build passed using
  the unchanged frozen lockfile.
- `make report-smoke`: all 11 tabs rendered cleanly.
- `make field-test`: `field test clean across 11 repositories`; both passes
  match each other and every committed baseline.
- `make field-audit`: completed across all 11 repositories; see the
  [completed worksheet](../../field-test/audit/2026-09-12-rust-dependency-refresh.md).
- `cargo package --list --allow-dirty`: includes Cargo.lock, rust-toolchain.toml,
  and both declared examples. This checks package contents, not a package build.

Native Linux was exercised locally. Windows and Alpine/musl release builds
remain CI validation; no claim of local execution is made for them.
