use assert_cmd::Command;

fn test_repo() -> String {
    std::env::var("BARAD_DUR_TEST_REPO").unwrap_or_else(|_| ".".to_string())
}

fn barad_dur() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("barad-dur").unwrap()
}

#[test]
fn analyze_current_dir_exits_zero() {
    barad_dur()
        .args(["analyze", &test_repo()])
        .assert()
        .success()
        .stdout(predicates::str::contains("Barad-dur"));
}

#[test]
fn analyze_json_is_valid() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("Output should be valid JSON");
    assert!(json["overall_score"].is_number());
    assert!(json["categories"].is_array());
    assert!(json["top_actions"].is_array());
}

#[test]
fn analyze_json_pretty_is_indented() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "--json", "--pretty"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("  \"overall_score\""));
}

#[test]
fn analyze_nonexistent_path_exits_nonzero() {
    barad_dur()
        .args(["analyze", "/tmp/nonexistent_barad_dur_test_path"])
        .assert()
        .failure();
}

#[test]
fn analyze_health_only_shows_health() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "--health", "-v"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Health"));
    // Should NOT contain other categories when --health is specified
    assert!(!text.contains("▸ Team"));
    assert!(!text.contains("▸ Evolution"));
    assert!(!text.contains("▸ Git Hygiene"));
}

#[test]
fn analyze_verbose_shows_metrics() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "-v"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Bus factor"));
}

#[test]
fn analyze_with_since_flag() {
    barad_dur()
        .args(["analyze", &test_repo(), "--since", "1month"])
        .assert()
        .success();
}

#[test]
fn analyze_output_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("report.json");

    barad_dur()
        .args([
            "analyze",
            &test_repo(),
            "--json",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output_path).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&content).expect("File should contain valid JSON");
    assert!(json["overall_score"].is_number());
}

#[test]
fn contributors_subcommand_exits_zero() {
    barad_dur()
        .args(["contributors", &test_repo()])
        .assert()
        .success();
}

#[test]
fn contributors_with_since_flag() {
    barad_dur()
        .args(["contributors", &test_repo(), "--since", "1month"])
        .assert()
        .success();
}

#[test]
fn analyze_with_exclude_ext_flag() {
    barad_dur()
        .args(["analyze", &test_repo(), "--exclude-ext", "rs"])
        .assert()
        .success();
}

#[test]
fn analyze_exclude_ext_multiple_flags() {
    barad_dur()
        .args([
            "analyze",
            &test_repo(),
            "--exclude-ext",
            "rs",
            "--exclude-ext",
            "toml",
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("overall_score"));
}

#[test]
fn analyze_json_and_html_are_mutually_exclusive() {
    barad_dur()
        .args(["analyze", &test_repo(), "--json", "--html"])
        .assert()
        .failure();
}

#[test]
fn analyze_verbose_shows_timing_in_stderr() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "-v", "--json", "--health"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("Metrics:"),
        "verbose should print metrics timing, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Scoring:"),
        "verbose should print scoring timing, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Render:"),
        "verbose should print render timing, stderr: {stderr}"
    );
}

#[test]
fn analyze_no_verbose_has_no_timing_in_stderr() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "--json", "--health"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !stderr.contains("Metrics:"),
        "non-verbose should not show timing, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Scoring:"),
        "non-verbose should not show timing, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Render:"),
        "non-verbose should not show timing, stderr: {stderr}"
    );
}

/// Create a throwaway git repo at `dir` with `files` (path, contents) and one
/// commit, so `barad-dur` has a tracked file tree to analyze.
fn init_repo(dir: &std::path::Path, files: &[(&str, &str)]) {
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("git should be on PATH");
        assert!(status.success(), "git {args:?} failed");
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test"]);
    for (name, contents) in files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
}

/// Run `analyze --json --no-cache` on `dir` and return the reported `total_files`
/// (which is the post-exclusion tracked-file count).
fn analyzed_total_files(dir: &std::path::Path) -> u64 {
    let output = barad_dur()
        .args(["analyze", dir.to_str().unwrap(), "--json", "--no-cache"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    json["total_files"]
        .as_u64()
        .expect("total_files should be a number")
}

#[test]
fn analyze_honors_baraddurignore_negation() {
    // `bundle.min.js` is dropped by the built-in `min.js` compound default. Both
    // repos are identical except the ignore file, so the only possible difference
    // in `total_files` is the negation re-including that one file.
    let baseline_dir = tempfile::tempdir().unwrap();
    init_repo(
        baseline_dir.path(),
        &[
            ("main.rs", "fn main() {}\n"),
            ("bundle.min.js", "console.log(1)\n"),
            (".baraddurignore", "# nothing re-included\n"),
        ],
    );
    let baseline = analyzed_total_files(baseline_dir.path());

    let negated_dir = tempfile::tempdir().unwrap();
    init_repo(
        negated_dir.path(),
        &[
            ("main.rs", "fn main() {}\n"),
            ("bundle.min.js", "console.log(1)\n"),
            (".baraddurignore", "!bundle.min.js\n"),
        ],
    );
    let with_negation = analyzed_total_files(negated_dir.path());

    assert_eq!(
        with_negation,
        baseline + 1,
        "`!bundle.min.js` should re-include exactly the default-excluded file"
    );
}

#[test]
fn gate_honors_baraddurignore() {
    // `gate` shares the collection path with `analyze`, so it must build the
    // `.baraddurignore` matcher too. Excluding `*.py` still leaves `main.rs`, so
    // the run stays meaningful; we only assert it exits cleanly (pass/fail a
    // threshold), never panicking.
    let dir = tempfile::tempdir().unwrap();
    init_repo(
        dir.path(),
        &[
            ("main.rs", "fn main() {}\n"),
            ("app.py", "print(1)\n"),
            (".baraddurignore", "*.py\n"),
        ],
    );
    let output = barad_dur()
        .args(["gate", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    let code = output.status.code();
    assert!(
        code == Some(0) || code == Some(1),
        "gate should exit 0 or 1, got {code:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn gate_passes_with_zero_min_score() {
    // Any score is >= 0, so the gate must succeed (exit 0).
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path(), &[("src/main.rs", "fn main() {}\n")]);
    barad_dur()
        .args(["gate", dir.path().to_str().unwrap(), "--min-score", "0"])
        .assert()
        .success();
}

#[test]
fn gate_fails_with_unreachable_min_score() {
    // A tiny repo cannot score 100, so the gate must fail (exit non-zero).
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path(), &[("src/main.rs", "fn main() {}\n")]);
    barad_dur()
        .args(["gate", dir.path().to_str().unwrap(), "--min-score", "100"])
        .assert()
        .failure();
}

#[test]
fn init_writes_config_and_baraddurignore() {
    // A repo with a translation file yields a detected exclude pattern, so `init`
    // writes both the TOML config and a `.baraddurignore` containing it. The .resx
    // is nested to guard against the shell-glob recount regression.
    let dir = tempfile::tempdir().unwrap();
    init_repo(
        dir.path(),
        &[
            ("src/main.rs", "fn main() {}\n"),
            ("src/resources/Strings.resx", "<x/>\n"),
        ],
    );
    barad_dur()
        .args(["init", dir.path().to_str().unwrap()])
        .assert()
        .success();

    let toml = std::fs::read_to_string(dir.path().join(".repository-analysis/barad-dur.toml"))
        .expect("config written");
    assert!(toml.contains("[analysis]"));
    assert!(!toml.contains("[exclude]"));

    let ignore = std::fs::read_to_string(dir.path().join(".baraddurignore"))
        .expect(".baraddurignore written");
    assert!(ignore.contains("*.resx"));
}

#[test]
fn init_without_force_fails_when_config_exists() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path(), &[("src/main.rs", "fn main() {}\n")]);
    let path = dir.path().to_str().unwrap();

    barad_dur().args(["init", path]).assert().success();
    // Re-running without --force must fail because the config now exists.
    barad_dur().args(["init", path]).assert().failure();
    // --force overwrites and succeeds.
    barad_dur()
        .args(["init", path, "--force"])
        .assert()
        .success();
}

#[test]
fn analyze_html_output_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("report.html");

    barad_dur()
        .args([
            "analyze",
            &test_repo(),
            "--html",
            "--health",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output_path).expect("report.html should be created");
    assert!(
        content.starts_with("<!DOCTYPE"),
        "HTML report should start with DOCTYPE"
    );
}

#[test]
fn analyze_json_has_expected_categories() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "--json", "--health"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let categories: Vec<&str> = json["categories"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();

    assert!(
        !categories.is_empty(),
        "expected at least one category in analysis output"
    );
    assert!(
        categories.iter().any(|&n| n.contains("Health")),
        "expected Health category, got: {categories:?}"
    );
}

#[test]
fn analyze_without_deps_flag_omits_deps_category() {
    let output = barad_dur()
        .args(["analyze", &test_repo(), "--json"])
        .output()
        .unwrap();

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let categories: Vec<&str> = json["categories"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();

    assert!(
        !categories.contains(&"Dependencies"),
        "Dependencies should not appear without --deps flag"
    );
}
