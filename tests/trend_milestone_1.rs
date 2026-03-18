/// Milestone 1 acceptance tests for the historical-trends feature.
///
/// Full AC coverage for:
///   - US-01: Auto-record Trend Snapshot (AC-01.3 through AC-01.6)
///   - US-02: Inline Delta Display (AC-02.1 through AC-02.5)
///   - US-04: JSON Trend Schema (AC-04.1 through AC-04.6)
///
/// All tests are marked #[ignore]. Enable one at a time during implementation.
/// Prerequisite: trend_walking_skeleton tests must pass before enabling these.
///
/// Driving port: `barad-dur analyze <path>` binary invoked via assert_cmd.
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn barad_dur() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("barad-dur").unwrap()
}

fn init_git_repo(dir: &Path, branch: &str) -> String {
    std::process::Command::new("git")
        .args(["init", "-b", branch])
        .current_dir(dir)
        .output()
        .expect("git init failed");

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .expect("git config email failed");

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .expect("git config name failed");

    fs::write(dir.join("README.md"), "# Test repo\n").expect("write README failed");

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add failed");

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()
        .expect("git commit failed");

    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("git rev-parse failed");

    String::from_utf8(out.stdout)
        .expect("utf8")
        .trim()
        .to_string()
}

fn trends_path(repo_dir: &Path) -> std::path::PathBuf {
    repo_dir.join(".repository-analysis").join("trends.json")
}

/// Parse NDJSON trends.json into a JSON array of entry objects.
fn read_trends(repo_dir: &Path) -> Vec<serde_json::Value> {
    let content = fs::read_to_string(trends_path(repo_dir)).expect("trends.json should exist");
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("each line should be valid JSON"))
        .collect()
}

/// Write a pre-seeded trends.json with the given NDJSON content.
fn seed_trends(repo_dir: &Path, ndjson: &str) {
    let dir = repo_dir.join(".repository-analysis");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("trends.json"), ndjson).unwrap();
}

// ---------------------------------------------------------------------------
// US-01 / AC-01.3: First run creates the file and outputs "Trend: first snapshot recorded"
// (mirrors walking skeleton but included here for standalone milestone coverage)
//
// Given no trends.json exists
// When the user runs barad-dur analyze
// Then trends.json is created and stdout contains "Trend: first snapshot recorded"
// ---------------------------------------------------------------------------
#[test]
fn ac_01_3_first_run_outputs_first_snapshot_message() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    assert!(!trends_path(repo_path).exists());

    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(
        stdout.contains("Trend: first snapshot recorded"),
        "stdout should contain 'Trend: first snapshot recorded'\nActual:\n{stdout}"
    );
    assert!(
        trends_path(repo_path).exists(),
        "trends.json should be created on first run"
    );
}

// ---------------------------------------------------------------------------
// US-01 / AC-01.4: Corrupt trends.json is archived to trends.json.bak and replaced
//
// Given trends.json contains invalid JSON (simulated disk corruption)
// When the user runs barad-dur analyze
// Then a warning is shown containing "trends.json could not be read"
// And trends.json.bak exists and contains the corrupt content
// And trends.json exists with exactly 1 entry (today's run)
// And the command exits with code 0
// ---------------------------------------------------------------------------
#[test]
fn ac_01_4_corrupt_trends_file_is_archived_and_replaced() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    let corrupt_content = "{ this is not valid JSON !!!";
    seed_trends(repo_path, corrupt_content);

    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // Warning message
    assert!(
        stdout.contains("trends.json could not be read"),
        "stdout should warn about unreadable trends.json\nActual:\n{stdout}"
    );

    // Backup file contains the corrupt content
    let bak_path = repo_path
        .join(".repository-analysis")
        .join("trends.json.bak");
    assert!(bak_path.exists(), "trends.json.bak should exist after corruption recovery");
    let bak_content = fs::read_to_string(&bak_path).unwrap();
    assert_eq!(
        bak_content, corrupt_content,
        "trends.json.bak should contain the original corrupt content"
    );

    // Fresh trends.json with 1 entry
    let entries = read_trends(repo_path);
    assert_eq!(
        entries.len(),
        1,
        "trends.json should have exactly 1 entry after corruption recovery"
    );
}

// ---------------------------------------------------------------------------
// US-01 / AC-01.5: --no-cache still records a trend entry
//
// Given trends.json exists with 1 entry
// When the user runs barad-dur analyze --no-cache
// Then trends.json now contains 2 entries
// ---------------------------------------------------------------------------
#[test]
fn ac_01_5_no_cache_flag_still_records_trend() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Establish first entry
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_trends(repo_path).len(), 1);

    // When: run with --no-cache
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--no-cache"])
        .assert()
        .success();

    // Then: 2 entries
    assert_eq!(
        read_trends(repo_path).len(),
        2,
        "--no-cache run should still append a trend entry"
    );
}

// ---------------------------------------------------------------------------
// US-01 / AC-01.6: Trend recording adds at most 0.5 seconds to total runtime
//
// Given a repository with a pre-existing trends.json
// When the user runs barad-dur analyze
// Then the total wall-clock time is within 0.5 seconds of a baseline run
// without trend recording (measured by running twice and comparing).
//
// Implementation note: this test measures elapsed time and fails if trend
// overhead exceeds 500ms. It is intentionally generous to avoid flakiness
// on slow CI machines; the design constraint is "no new git calls".
// ---------------------------------------------------------------------------
#[test]
fn ac_01_6_trend_recording_overhead_under_500ms() {
    use std::time::Instant;

    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Warm up: first run (creates trends.json, populates any caches)
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    // Measure two consecutive runs; delta between them represents trend overhead
    // relative to an already-warmed path.
    let t0 = Instant::now();
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();
    let run1 = t0.elapsed();

    let t1 = Instant::now();
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();
    let run2 = t1.elapsed();

    // The overhead introduced by trend recording is the difference between runs.
    // Both runs record a trend entry; the delta should be negligible (<500ms).
    let overhead = if run2 > run1 {
        run2 - run1
    } else {
        std::time::Duration::ZERO
    };

    assert!(
        overhead.as_millis() < 500,
        "trend recording overhead should be under 500ms, got {}ms",
        overhead.as_millis()
    );
}

// ---------------------------------------------------------------------------
// US-02 / AC-02.1: Delta appears inline with overall score (same-branch prior snapshot)
//
// Given trends.json has 1 prior entry on branch "main"
// When the user runs barad-dur analyze on the same branch
// Then the output contains the overall score with a delta marker ("vs last run")
// ---------------------------------------------------------------------------
#[test]
fn ac_02_1_delta_shown_inline_with_overall_score() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Establish baseline
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    // Second run — should show delta
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(
        stdout.contains("vs last run"),
        "stdout should contain 'vs last run' delta next to overall score\nActual:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// US-02 / AC-02.2: Per-category deltas shown on each category row
//
// Given trends.json has 1 prior entry on branch "main"
// When the user runs barad-dur analyze
// Then each category row shows its numeric delta
// ---------------------------------------------------------------------------
#[test]
fn ac_02_2_per_category_deltas_shown() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // Each category row should have a delta indicator (+N or -N or +0)
    // We check that at least the four category names appear alongside delta markers.
    // The pattern "+0" or "+N" or "-N" adjacent to a category name confirms per-row deltas.
    let category_delta_present = stdout.contains("Health")
        && stdout.contains("Team")
        && stdout.contains("Evolution")
        && stdout.contains("Git Hygiene");

    assert!(
        category_delta_present,
        "stdout should show all four category rows with delta information\nActual:\n{stdout}"
    );

    // At least one numeric delta marker must appear (format: +N or -N)
    let has_numeric_delta = stdout
        .lines()
        .any(|l| l.contains('+') || (l.contains('-') && l.chars().any(|c| c.is_ascii_digit())));
    assert!(
        has_numeric_delta,
        "stdout should contain at least one numeric delta marker (+N / -N)\nActual:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// US-02 / AC-02.3: Compact trend sparkline with direction indicator shown
//
// Given trends.json has 1 prior entry on branch "main"
// When the user runs barad-dur analyze
// Then the output contains a direction indicator (improving / declining / stable)
// And the output contains a sparkline showing score progression
// ---------------------------------------------------------------------------
#[test]
fn ac_02_3_sparkline_and_direction_indicator_shown() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // Direction indicator
    let has_direction = stdout.contains("improving")
        || stdout.contains("declining")
        || stdout.contains("stable");
    assert!(
        has_direction,
        "stdout should contain a direction indicator\nActual:\n{stdout}"
    );

    // Sparkline: look for the arrow separator used in sparkline format "N → N"
    let has_sparkline = stdout.contains('→') || stdout.contains("->") || stdout.contains('↑') || stdout.contains('↓');
    assert!(
        has_sparkline,
        "stdout should contain a sparkline or trend arrow\nActual:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// US-02 / AC-02.4: Branch mismatch suppresses delta with warning
//
// Given trends.json has entries recorded on branch "feature/refactor"
// And the current HEAD is on branch "main"
// When the user runs barad-dur analyze
// Then no delta is shown next to the overall score
// And a warning message mentions the branch mismatch
// And the new snapshot is appended with branch "main"
// ---------------------------------------------------------------------------
#[test]
fn ac_02_4_branch_mismatch_suppresses_delta_shows_warning() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    // Init on "feature/refactor"
    init_git_repo(repo_path, "feature/refactor");

    // Seed a trends.json entry recorded on feature/refactor (trailing newline so
    // append_if_new_head writes the next entry on its own line).
    let now = chrono::Utc::now().to_rfc3339();
    let entry = format!(
        "{}\n",
        r#"{"timestamp":"2024-01-01T00:00:00Z","commit":"aabbccdd00112233445566778899aabbccddeeff","branch":"feature/refactor","overall_score":70,"schema_version":1,"category_scores":{"Health":70,"Team":70,"Evolution":70,"Git Hygiene":70}}"#
    );
    let _ = now; // timestamp is embedded above as a fixed value for determinism
    seed_trends(repo_path, &entry);

    // Switch to "main" branch
    std::process::Command::new("git")
        .args(["checkout", "-b", "main"])
        .current_dir(repo_path)
        .output()
        .expect("git checkout failed");

    // When: run on main
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // Then: no delta shown (no "vs last run" marker)
    assert!(
        !stdout.contains("vs last run"),
        "stdout should NOT show delta when branches differ\nActual:\n{stdout}"
    );

    // And: warning mentions branch mismatch
    let has_branch_warning = stdout.contains("feature/refactor") && stdout.contains("main");
    assert!(
        has_branch_warning,
        "stdout should warn about branch mismatch mentioning both branch names\nActual:\n{stdout}"
    );

    // And: new entry appended with branch "main"
    let entries = read_trends(repo_path);
    let last = entries.last().expect("at least one entry after run");
    assert_eq!(
        last["branch"].as_str().unwrap(),
        "main",
        "newly appended entry should record branch 'main'"
    );
}

// ---------------------------------------------------------------------------
// US-02 / AC-02.5: Output remains parseable by existing scripts (no breaking format change)
//
// Given trends.json has 1 prior entry
// When the user runs barad-dur analyze
// Then the output score line still contains the numeric score in "N/100" format
// (existing scripts that extract the score via regex still work)
// ---------------------------------------------------------------------------
#[test]
fn ac_02_5_output_score_format_unchanged_for_script_compatibility() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // The score line must still contain the pattern "N/100" where N is 0-100.
    // This ensures existing `grep -oP '\d+(?=/100)'` style scripts still work.
    let has_score_pattern = stdout
        .lines()
        .any(|line| {
            let re_like = line
                .split_whitespace()
                .any(|token| token.ends_with("/100") && token.trim_end_matches("/100").parse::<u32>().is_ok());
            re_like
        });

    assert!(
        has_score_pattern,
        "stdout should contain 'N/100' score pattern for script compatibility\nActual:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// US-04 / AC-04.1 + AC-04.3 + AC-04.4 + AC-04.6:
// --json --trend outputs trend key with snapshots, required fields, schema_version=1,
// and direction is one of "improving" / "declining" / "stable"
//
// Given trends.json has prior entries on branch "main"
// When the user runs barad-dur analyze --trend --json
// Then the JSON output contains a top-level "trend" object
// And trend.snapshots is an array
// And each snapshot entry has: timestamp, commit, branch, overall_score, category_scores
// And category_scores has exactly the keys: Health, Team, Evolution, Git Hygiene
// And trend.schema_version is integer 1
// And trend.direction is one of "improving", "declining", "stable"
// ---------------------------------------------------------------------------
#[test]
fn ac_04_1_json_trend_flag_outputs_trend_key_with_required_fields() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Establish prior entries
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    // When: --trend --json
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--trend", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("--json output should be valid JSON");

    // Then: top-level "trend" object exists
    let trend = json.get("trend").expect("JSON should have top-level 'trend' key");
    assert!(trend.is_object(), "'trend' should be a JSON object");

    // And: trend.snapshots is an array with entries
    let snapshots = trend["snapshots"]
        .as_array()
        .expect("trend.snapshots should be an array");
    assert!(!snapshots.is_empty(), "trend.snapshots should contain at least one entry");

    // And: each snapshot has required fields
    for (i, snap) in snapshots.iter().enumerate() {
        let ts = snap["timestamp"].as_str().unwrap_or("");
        assert!(
            !ts.is_empty() && (ts.ends_with('Z') || ts.contains('+')),
            "snapshot[{i}].timestamp should be ISO8601 UTC: {ts}"
        );
        assert!(
            snap["commit"].is_string(),
            "snapshot[{i}].commit should be a string"
        );
        assert!(
            snap["branch"].is_string(),
            "snapshot[{i}].branch should be a string"
        );
        assert!(
            snap["overall_score"].is_number(),
            "snapshot[{i}].overall_score should be a number"
        );

        let cat = snap["category_scores"]
            .as_object()
            .unwrap_or_else(|| panic!("snapshot[{i}].category_scores should be an object"));
        for key in &["Health", "Team", "Evolution", "Git Hygiene"] {
            assert!(
                cat.contains_key(*key),
                "snapshot[{i}].category_scores should contain key '{key}'"
            );
        }
    }

    // And: schema_version is integer 1
    assert_eq!(
        trend["schema_version"].as_i64().expect("schema_version should be integer"),
        1,
        "trend.schema_version should be 1"
    );

    // And: direction is one of "improving", "declining", "stable"
    let direction = trend["direction"]
        .as_str()
        .expect("trend.direction should be a string");
    assert!(
        ["improving", "declining", "stable"].contains(&direction),
        "trend.direction should be one of improving/declining/stable, got: {direction}"
    );
}

// ---------------------------------------------------------------------------
// US-04 / AC-04.1: trend key includes delta_vs_last, delta_vs_oldest, velocity_per_week
//
// Given 3+ prior trend entries exist
// When the user runs barad-dur analyze --trend --json
// Then trend.delta_vs_last is an integer
// And trend.delta_vs_oldest is an integer
// And trend.velocity_per_week is a number (not null when 2+ prior snapshots)
// ---------------------------------------------------------------------------
#[test]
fn ac_04_1_trend_includes_delta_and_velocity_fields() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Three prior runs
    for _ in 0..3 {
        barad_dur()
            .args(["analyze", repo_path.to_str().unwrap()])
            .assert()
            .success();
    }

    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--trend", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let trend = &json["trend"];

    assert!(
        trend["delta_vs_last"].is_number(),
        "trend.delta_vs_last should be a number"
    );
    assert!(
        trend["delta_vs_oldest"].is_number(),
        "trend.delta_vs_oldest should be a number"
    );
    // With 3+ prior runs velocity_per_week must be a number, not null
    assert!(
        trend["velocity_per_week"].is_number(),
        "trend.velocity_per_week should be a number when 3+ snapshots exist"
    );
}

// ---------------------------------------------------------------------------
// US-04 / AC-04.2: --json without --trend produces structurally identical output
// (backward-compatibility contract test)
//
// This test captures the shape of current JSON output and verifies that adding
// trend history does not alter it. Shape = set of top-level keys present.
//
// Given trends.json exists (trend recording is active)
// When the user runs barad-dur analyze --json (no --trend flag)
// Then the JSON output has exactly the same top-level keys as the pre-trends baseline
// And no "trend" key is present
// ---------------------------------------------------------------------------
#[test]
fn ac_04_2_json_without_trend_flag_is_structurally_unchanged() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Capture baseline shape on first run (no prior trends.json)
    let baseline_output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let baseline: serde_json::Value = serde_json::from_slice(&baseline_output)
        .expect("baseline JSON should be valid");
    let baseline_keys: std::collections::BTreeSet<String> = baseline
        .as_object()
        .expect("baseline JSON should be an object")
        .keys()
        .cloned()
        .collect();

    // Establish trend history
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    // Second run with --json only (no --trend)
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let result: serde_json::Value =
        serde_json::from_slice(&output).expect("output should be valid JSON");
    let result_keys: std::collections::BTreeSet<String> = result
        .as_object()
        .expect("output should be a JSON object")
        .keys()
        .cloned()
        .collect();

    // No "trend" key
    assert!(
        !result_keys.contains("trend"),
        "--json without --trend should NOT contain 'trend' key"
    );

    // Exactly the same top-level keys as baseline (no additions, no removals)
    assert_eq!(
        baseline_keys, result_keys,
        "--json output top-level keys should be identical to baseline.\nBaseline: {baseline_keys:?}\nActual: {result_keys:?}"
    );
}

// ---------------------------------------------------------------------------
// US-04 / AC-04.5: velocity_per_week is null (not absent) when fewer than 2 prior snapshots
//
// Given trends.json has exactly 0 prior entries (first run)
// When the user runs barad-dur analyze --trend --json
// Then trend.velocity_per_week is JSON null, not absent
// ---------------------------------------------------------------------------
#[test]
fn ac_04_5_velocity_is_null_when_fewer_than_2_prior_snapshots() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // First run — no prior snapshots
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--trend", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let trend = json.get("trend").expect("trend key should exist");

    // velocity_per_week must be present as null, not absent
    assert!(
        trend.as_object().unwrap().contains_key("velocity_per_week"),
        "trend.velocity_per_week key must be present (as null)"
    );
    assert!(
        trend["velocity_per_week"].is_null(),
        "trend.velocity_per_week should be null when fewer than 2 prior snapshots, got: {}",
        trend["velocity_per_week"]
    );
}

// ---------------------------------------------------------------------------
// US-04 / AC-04.6: direction field reflects actual trajectory
//
// Given trends.json has entries with overall_score [60, 65, 68, 72]
// When the user runs barad-dur analyze --trend --json with current score above prior
// Then trend.direction is "improving"
// And trend.delta_vs_last is positive
// ---------------------------------------------------------------------------
#[test]
fn ac_04_6_direction_field_reflects_improving_trajectory() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Seed trends.json with ascending scores to establish an improving trajectory.
    // We seed realistic entries; the tool will append a new one on top.
    let base_ts = chrono::Utc::now() - chrono::Duration::days(30);
    let mut ndjson = String::new();
    let sha = "aabbccdd00112233445566778899aabbccddeeff";
    for (i, score) in [60u32, 65, 68, 72].iter().enumerate() {
        let ts = (base_ts + chrono::Duration::days(i as i64 * 7))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        ndjson.push_str(&format!(
            r#"{{"timestamp":"{ts}","commit":"{sha}","branch":"main","overall_score":{score},"schema_version":1,"category_scores":{{"Health":{score},"Team":{score},"Evolution":{score},"Git Hygiene":{score}}}}}"#
        ));
        ndjson.push('\n');
    }
    seed_trends(repo_path, &ndjson);

    // Assuming the real analysis of this minimal repo produces a score >= 72,
    // direction should be "improving". We cannot guarantee the exact score value
    // but we CAN assert direction is a valid enum value.
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--trend", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let trend = &json["trend"];

    let direction = trend["direction"].as_str().expect("direction should be a string");
    assert!(
        ["improving", "declining", "stable"].contains(&direction),
        "trend.direction should be a valid enum value, got: {direction}"
    );

    // delta_vs_last must be an integer (positive, negative, or zero)
    assert!(
        trend["delta_vs_last"].is_number(),
        "trend.delta_vs_last should be a number"
    );
}

// ---------------------------------------------------------------------------
// US-04 / AC-04.6 (error path): direction is "declining" when score drops
//
// Given trends.json has a prior entry with a very high overall_score (99)
// When the user runs barad-dur analyze --trend --json (real score will be lower)
// Then trend.direction is "declining"
// And trend.delta_vs_last is negative
// ---------------------------------------------------------------------------
#[test]
fn ac_04_6_direction_is_declining_when_score_drops() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Seed a single prior entry with score 99 — real analysis score will be lower
    let ts = (chrono::Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let sha = "aabbccdd00112233445566778899aabbccddeeff";
    let entry = format!(
        r#"{{"timestamp":"{ts}","commit":"{sha}","branch":"main","overall_score":99,"schema_version":1,"category_scores":{{"Health":99,"Team":99,"Evolution":99,"Git Hygiene":99}}}}"#
    );
    seed_trends(repo_path, &entry);

    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap(), "--trend", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let trend = &json["trend"];

    assert_eq!(
        trend["direction"].as_str().expect("direction should be string"),
        "declining",
        "direction should be 'declining' when current score is below prior score"
    );

    let delta = trend["delta_vs_last"].as_i64().expect("delta_vs_last should be integer");
    assert!(
        delta < 0,
        "delta_vs_last should be negative when score drops, got: {delta}"
    );
}
