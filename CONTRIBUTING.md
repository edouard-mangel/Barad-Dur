# Contributing to Barad-dur

## Getting started

```bash
git clone <repo-url>
cd barad-dur
cargo build
cargo test
```

**System dependencies** (for libgit2):
```bash
# Debian/Ubuntu
sudo apt install build-essential cmake pkg-config libssl-dev

# macOS
brew install cmake pkg-config openssl
```

## Development workflow

```bash
cargo test                          # all tests (unit + integration)
cargo clippy --all-targets -- -D warnings
cargo fmt

# Dogfood — analyze the repo itself
cargo run -- analyze . -v
cargo run -- analyze . --html -o report.html
```

## Code conventions

This project follows a **functional programming** style in Rust:

- Pure functions: no side-effects, no mutation of inputs
- Iterator chains (`.map()`, `.filter()`, `.fold()`) over mutable loops
- Error propagation with `?` and `Result<T, E>`
- Immutable bindings by default — add `mut` only when required
- New metrics follow the `(snapshot) → value` pattern from `src/metrics/`

## Adding a metric

1. Add a pure function in the appropriate `src/metrics/<module>.rs`
2. Return a `MetricValue` (score 0–100 + optional detail)
3. Register it in `src/scorer.rs` → `build_report()`
4. Add unit tests using the helpers in `src/metrics/testutil.rs`

The metric receives a `&RepoSnapshot` and must not perform I/O.

## Testing

- Unit tests live alongside the code in `src/`
- Integration tests are in `tests/`
- Aim for ≥ 80% mutation kill rate (checked on CI)

## Submitting changes

### Via GitHub (recommended for external contributors)

1. Fork the repo on GitHub and create a branch
2. Make your change with tests
3. Run `cargo test && cargo clippy && cargo fmt --check`
4. Open a **GitHub PR** — a bot will automatically:
   - Push your branch to Froggit (the primary CI host)
   - Open a Froggit MR and post the link as a PR comment
5. Full CI runs on Froggit: mutation testing (≥ 80% kill rate), coverage (≥ 80%), SAST
6. The maintainer reviews and merges the **Froggit MR** — your GitHub PR closes automatically

> Do not merge directly on GitHub. The Froggit pipeline is the authoritative gate.

### Via Froggit (for maintainers)

Open a merge request directly on [lab.frogg.it](https://lab.frogg.it/Edouard_Mangel/barad-dur).

## License

By contributing, you agree your changes will be licensed under GPL-3.0-only.
