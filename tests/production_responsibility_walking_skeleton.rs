use barad_dur::{
    config::HealthThresholds,
    metrics::{complexity, health},
    snapshot::{RepoSnapshot, TimeWindow},
};
use serde_json::Value;
use std::path::Path;
use tempfile::TempDir;
mod common;
use common::{barad_dur, git};

#[test]
fn responsibility_advice_ignores_inline_tests_on_cold_warm_and_forced_runs() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    let padding = (0..510)
        .map(|i| format!("let _value_{i} = {i};\n"))
        .collect::<String>();
    let source = format!("pub fn validate_input() {{\n{padding}if true {{}}\n}}\n#[cfg(test)] mod checks {{ #[test] fn validate_empty() {{}} #[test] fn validate_valid() {{}} }}\n");
    std::fs::write(dir.path().join("src/lib.rs"), &source).unwrap();
    git(dir.path(), &["add", "src/lib.rs"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "Add mixed production and test fixture",
        ],
    );
    let mut snapshot = RepoSnapshot::new(
        dir.path().to_path_buf(),
        "fixture".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.file_metrics.insert(
        "src/lib.rs".into(),
        complexity::analyse_source(Path::new("src/lib.rs"), &source).metrics,
    );
    assert_eq!(
        health::god_object_files(&snapshot, &HealthThresholds::default()).len(),
        1
    );
    let analyze = |no_cache| -> Value {
        let mut command = barad_dur();
        command
            .arg("analyze")
            .arg(dir.path())
            .args(["--json", "--all", "--health"]);
        if no_cache {
            command.arg("--no-cache");
        }
        let output = command.assert().success().get_output().stdout.clone();
        serde_json::from_slice(&output).unwrap()
    };
    let cold = analyze(false);
    let warm = analyze(false);
    let forced = analyze(true);
    for report in [&cold, &warm, &forced] {
        let actions = report["top_actions"].as_array().unwrap();
        assert!(
            actions.iter().all(|action| !action["text"]
                .as_str()
                .unwrap()
                .contains("consider splitting by responsibility")),
            "{actions:#?}"
        );
    }
    assert_eq!(cold["top_actions"], warm["top_actions"]);
    assert_eq!(cold["top_actions"], forced["top_actions"]);
    assert_eq!(cold["categories"], warm["categories"]);
    assert_eq!(cold["categories"], forced["categories"]);
}

#[test]
fn structural_responsibility_advice_is_identical_on_cold_warm_and_forced_runs() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    let padding = (0..510)
        .map(|i| format!("let _value_{i} = {i};\n"))
        .collect::<String>();
    let source = format!(
        r#"
struct State {{ ready: bool }}
impl State {{
    fn is_ready(&self) -> bool {{ self.ready }}
    fn is_active(&self) -> bool {{ self.ready }}
    fn has_ready(&self) -> bool {{ self.ready }}
    fn render_first(&self) {{}}
    fn render_second(&self) {{}}
}}
impl State {{ fn has_other(&self) -> bool {{ self.ready }} }}
fn has_external() -> bool {{ external() }}
fn has_unrelated() -> bool {{ unrelated() }}
fn padding() {{
{padding}
if true {{}}
}}
"#
    );
    std::fs::write(dir.path().join("src/lib.rs"), &source).unwrap();
    git(dir.path(), &["add", "src/lib.rs"]);
    git(
        dir.path(),
        &["commit", "-q", "-m", "Add structural advice fixture"],
    );
    let analyze = |no_cache| -> Value {
        let mut command = barad_dur();
        command
            .arg("analyze")
            .arg(dir.path())
            .args(["--json", "--all", "--health"]);
        if no_cache {
            command.arg("--no-cache");
        }
        let output = command.assert().success().get_output().stdout.clone();
        serde_json::from_slice(&output).unwrap()
    };
    let cold = analyze(false);
    let warm = analyze(false);
    let forced = analyze(true);
    let advice = cold["top_actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["text"].as_str())
        .find(|text| text.contains("consider splitting by responsibility"))
        .expect("same-owner production groups must produce advice");
    // The whole structural segment is pinned: the owner, both of its groups and
    // the field evidence belong to the same declaration, and nothing else groups.
    assert!(
        advice.ends_with(
            "consider splitting by responsibility: State (line 3): is_* (2) shared field ready, render_* (2)"
        ),
        "{advice}"
    );
    assert_eq!(cold["top_actions"], warm["top_actions"]);
    assert_eq!(cold["top_actions"], forced["top_actions"]);
    assert_eq!(cold["categories"], warm["categories"]);
    assert_eq!(cold["categories"], forced["categories"]);
    let cached = barad_dur::cache::load(dir.path()).unwrap().unwrap();
    assert!(cached.file_metrics[Path::new("src/lib.rs")]
        .functions
        .iter()
        .find(|f| f.name == "is_ready")
        .unwrap()
        .responsibility
        .as_ref()
        .is_some_and(|p| !p.dependencies.is_empty()));
}
