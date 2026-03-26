/// Walking Skeleton tests for the multi-repository-analysis feature.
///
/// These tests form the outer loop of Outside-In TDD. They verify
/// the coupling subcommand is recognized by the CLI and parses
/// arguments correctly.
///
/// Driving port: `barad-dur coupling <root-dir>` binary invoked via assert_cmd.
/// No internal Rust components are called directly.
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// AC: coupling_cli_parses_subcommand
//
// GIVEN a user runs `barad-dur coupling <root-dir>`
// WHEN the root-dir argument is provided
// THEN the CLI parses the coupling subcommand without error
//   AND the command exits with code 0
// ---------------------------------------------------------------------------
#[test]
fn coupling_cli_parses_subcommand() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("barad-dur")
        .unwrap()
        .arg("coupling")
        .arg(temp.path())
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// AC: coupling_cli_requires_root_dir
//
// GIVEN a user runs `barad-dur coupling` without arguments
// THEN an error message indicates the root-dir is required
// ---------------------------------------------------------------------------
#[test]
fn coupling_cli_requires_root_dir() {
    Command::cargo_bin("barad-dur")
        .unwrap()
        .arg("coupling")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ---------------------------------------------------------------------------
// AC: existing_subcommands_unchanged
//
// GIVEN existing subcommands (analyze, backfill, init, gate)
// WHEN any is invoked
// THEN behavior is unchanged (help text still lists them)
// ---------------------------------------------------------------------------
#[test]
fn existing_subcommands_unchanged() {
    // All existing subcommands still appear in help
    Command::cargo_bin("barad-dur")
        .unwrap()
        .arg("--help")
        .assert()
        .stdout(predicate::str::contains("analyze"))
        .stdout(predicate::str::contains("backfill"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("gate"))
        .stdout(predicate::str::contains("coupling"));
}
