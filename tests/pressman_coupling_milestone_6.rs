//! M6 milestone E2E: a fixture with content, common, and control findings
//! across files produces a `coupling_actions` list ordered content→common→
//! control, each with kind-specific advice, surfaced through `analyze --json`.

use std::process::{Command, Output};

fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let git = |args: &[&str]| -> Output {
        let out = Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "f@e.com"]);
    git(&["config", "user.name", "F"]);
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    // common: static mut
    std::fs::write(
        dir.path().join("src/globals.rs"),
        "pub static mut COUNTER: usize = 0;\n",
    )
    .unwrap();
    // control: pub fn with a branched-on bool flag
    std::fs::write(
        dir.path().join("src/flags.rs"),
        "pub fn run(verbose: bool) -> u32 { if verbose { 1 } else { 0 } }\n",
    )
    .unwrap();
    // content: #[path] attribute import
    std::fs::write(dir.path().join("src/other.rs"), "pub fn h() {}\n").unwrap();
    std::fs::write(
        dir.path().join("src/hack.rs"),
        "#[path = \"other.rs\"]\nmod other;\npub fn g() { other::h() }\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    dir
}

fn coupling_action_texts(report: &serde_json::Value) -> Vec<String> {
    report["coupling_actions"]
        .as_array()
        .expect("coupling_actions array")
        .iter()
        .map(|a| a["text"].as_str().expect("text").to_string())
        .collect()
}

#[test]
fn coupling_actions_surface_ordered_and_kind_specific() {
    let dir = fixture_repo();
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir.path())
        .args(["--json", "--no-cache"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let texts = coupling_action_texts(&report);

    let content_i = texts
        .iter()
        .position(|t| t.contains("worst: content"))
        .expect("content action");
    let common_i = texts
        .iter()
        .position(|t| t.contains("worst: common"))
        .expect("common action");
    let control_i = texts
        .iter()
        .position(|t| t.contains("worst: control"))
        .expect("control action");
    assert!(
        content_i < common_i && common_i < control_i,
        "order: {texts:?}"
    );
    assert!(texts[content_i].contains("public interface"));
    assert!(texts[common_i].contains("injected state"));
    assert!(texts[control_i].contains("intent-revealing"));
}
