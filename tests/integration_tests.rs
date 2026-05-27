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
