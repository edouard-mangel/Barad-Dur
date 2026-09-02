//! Field-test harness driver. Not shipped: gated behind the `field-test`
//! cargo feature so `cargo install` never picks it up.
//!
//! Deliberately thin. Because this file is feature-gated it is invisible to a
//! plain `cargo test`, so every decision it makes lives in
//! `barad_dur::field_test::sweep`, which is compiled and tested
//! unconditionally. What is left here is I/O and sequencing.

use anyhow::{bail, Context, Result};
use barad_dur::field_test::{
    audit::render_worksheet,
    baseline::write_baseline,
    corpus::{parse_corpus, resolve_path},
    diff::diff_surfaces,
    mode::{parse_mode, Mode},
    runner::analyze_pinned,
    sweep::{audit_corpus, baseline_for, step_for, summary_line, AuditInput, RepoStep},
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Pre-existing recommendations sampled per merge, **across the whole
/// corpus** — not per repository. See `sweep::audit_corpus`.
const ROTATION: usize = 5;

fn corpus_root() -> Result<PathBuf> {
    if let Some(root) = std::env::var_os("BARAD_DUR_CORPUS_ROOT") {
        return Ok(PathBuf::from(root));
    }
    let home = std::env::var("HOME").context(
        "resolving corpus root: HOME is not set and BARAD_DUR_CORPUS_ROOT is not set either",
    )?;
    Ok(PathBuf::from(home).join("WS"))
}

/// Exit code 2 (harness error) is only reachable if this returns `Err` —
/// see `main`, which is the only place `std::process::exit` is called for
/// a non-zero-non-one code.
fn run() -> Result<()> {
    let mode = parse_mode(&std::env::args().nth(1).unwrap_or_else(|| "run".to_string()))?;
    let root = corpus_root()?;
    let baselines = Path::new("field-test/baselines");
    let archive = Path::new("field-test/archive");
    std::fs::create_dir_all(archive)
        .with_context(|| format!("creating archive directory {}", archive.display()))?;

    let entries = parse_corpus(
        &std::fs::read_to_string("field-test/corpus.toml")
            .context("reading field-test/corpus.toml")?,
    )?;
    let binary = PathBuf::from("target/release/barad-dur");
    let passes = if mode == Mode::Run { 2 } else { 1 };

    let mut failures = 0usize;
    let mut compared = 0usize;
    let mut accepted = 0usize;
    let mut audit_inputs: Vec<AuditInput> = Vec::new();

    for entry in &entries {
        let repo = resolve_path(entry, &root);
        let outcome = analyze_pinned(&binary, &entry.name, &repo, &entry.pin, archive, passes)?;

        if let Some(nd) = &outcome.nondeterminism {
            println!("NONDETERMINISM {}:\n{}", entry.name, nd.render());
            failures += 1;
        }

        match step_for(mode, baseline_for(mode, baselines, &entry.name)?) {
            RepoStep::Accept => {
                write_baseline(baselines, &entry.name, &outcome.surface)?;
                accepted += 1;
            }
            RepoStep::MissingBaseline => {
                bail!(
                    "missing committed baseline for {}; run `make field-test-accept` and commit the baseline as its own reviewed change",
                    entry.name
                );
            }
            // Audit never writes a baseline — `field-test-accept` is the only
            // path allowed to change one, so that every change to what the
            // tool recommends lands as its own reviewed commit.
            RepoStep::Audit(baseline) => audit_inputs.push(AuditInput {
                name: entry.name.clone(),
                baseline,
                current: outcome.surface,
            }),
            RepoStep::Compare(base) => {
                let d = diff_surfaces(&base, &outcome.surface);
                if !d.is_empty() {
                    println!("CHANGED {}:\n{}", entry.name, d.render());
                    failures += 1;
                }
                compared += 1;
            }
        }
    }

    if mode == Mode::Audit {
        // Safe to drop `failures` here without checking it: audit always
        // runs with `passes == 1` (see above), so `outcome.nondeterminism`
        // is always `None` and `failures` can only be incremented by the
        // `CHANGED` arm, which audit mode never reaches. If audit ever
        // gains a second pass, this early return would start silently
        // swallowing nondeterminism failures — revisit this invariant then.
        let sampled = audit_corpus(&audit_inputs, &BTreeSet::new(), ROTATION);
        if sampled.is_empty() {
            println!("field-test audit: nothing sampled across {} repositories (no new/changed recommendations, nothing due for rotation)", entries.len());
        } else {
            let worksheet: String = sampled
                .iter()
                .map(|(name, items)| format!("{}\n", render_worksheet(name, items)))
                .collect();
            print!("{worksheet}");
        }
        return Ok(());
    }

    if mode == Mode::Accept {
        println!(
            "field-test accept: rewrote {accepted} baselines — \
             commit the diff as its own reviewed commit"
        );
        return Ok(());
    }

    if failures > 0 {
        eprintln!("\n{failures} repository/repositories differ from baseline.");
        eprintln!("Explain each change in the review, or run `make field-test-accept`.");
        std::process::exit(1);
    }
    println!("{}", summary_line(compared));
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(2);
    }
}
