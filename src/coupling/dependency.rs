use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A direct dependency from one repo to another (path or git reference).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectDep {
    pub from: String,
    pub to: String,
}

/// A pair of repos with their shared dependency information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCouplingPair {
    pub repo_a: String,
    pub repo_b: String,
    pub shared_deps: Vec<String>,
    pub shared_count: usize,
    pub dep_score: f64,
    pub direct_dependency: Option<DirectDep>,
}

/// A dependency consumed by 3+ repos (hub dependency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusEntry {
    pub dependency_name: String,
    pub consumers: Vec<String>,
    pub consumer_count: usize,
}

/// Full result of dependency coupling analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAnalysis {
    pub pairs: Vec<DependencyCouplingPair>,
    pub blast_radius: Vec<BlastRadiusEntry>,
}

// ---------------------------------------------------------------------------
// Manifest file types
// ---------------------------------------------------------------------------

/// A Cargo.toml dependency that references another repo (via path or git).
#[derive(Debug, Clone)]
struct CargoDep {
    /// The crate/repo name this dependency references.
    references_repo: String,
}

// ---------------------------------------------------------------------------
// Manifest parsing — pure functions
// ---------------------------------------------------------------------------

/// Parse dependency names from a Cargo.toml file content.
/// Returns (regular dep names, direct-dep references to other repos).
fn parse_cargo_toml(content: &str) -> (Vec<String>, Vec<CargoDep>) {
    let parsed: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let mut dep_names = Vec::new();
    let mut direct_deps = Vec::new();

    let sections = ["dependencies", "dev-dependencies"];
    for section in &sections {
        if let Some(table) = parsed.get(section).and_then(|v| v.as_table()) {
            for (name, value) in table {
                let has_path = value.get("path").is_some();
                let has_git = value.get("git").is_some();

                if has_path || has_git {
                    direct_deps.push(CargoDep {
                        references_repo: name.clone(),
                    });
                } else {
                    dep_names.push(name.clone());
                }
            }
        }
    }

    (dep_names, direct_deps)
}

/// Parse dependency names from a package.json file content.
fn parse_package_json(content: &str) -> Vec<String> {
    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let sections = ["dependencies", "devDependencies"];
    sections
        .iter()
        .flat_map(|section| {
            parsed
                .get(section)
                .and_then(|v| v.as_object())
                .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        })
        .collect()
}

/// Parse dependency names from a go.mod file content.
fn parse_go_mod(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_require_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("require (") || trimmed == "require (" {
            in_require_block = true;
            continue;
        }

        if in_require_block {
            if trimmed == ")" {
                in_require_block = false;
                continue;
            }
            // Lines like: github.com/gin-gonic/gin v1.9.0
            if let Some(module_path) = trimmed.split_whitespace().next() {
                if !module_path.is_empty() {
                    deps.push(module_path.to_string());
                }
            }
        }

        // Single-line require: require github.com/foo/bar v1.0.0
        if trimmed.starts_with("require ") && !trimmed.contains('(') {
            let rest = trimmed.strip_prefix("require ").unwrap_or("");
            if let Some(module_path) = rest.split_whitespace().next() {
                if !module_path.is_empty() {
                    deps.push(module_path.to_string());
                }
            }
        }
    }

    deps
}

/// Parse dependency names from a requirements.txt file content.
fn parse_requirements_txt(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('-'))
        .filter_map(|line| {
            // Split on version specifiers: ==, >=, <=, !=, ~=, >, <, [
            let name = line
                .split(&['=', '>', '<', '!', '~', '['][..])
                .next()
                .map(|s| s.trim().to_lowercase());
            name.filter(|n| !n.is_empty())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Repo dependency extraction — reads filesystem
// ---------------------------------------------------------------------------

/// Information about a single repo's dependencies (parsed from manifests).
#[derive(Debug)]
struct RepoDeps {
    name: String,
    deps: HashSet<String>,
    /// Direct references to other repos (from Cargo.toml path/git deps).
    direct_refs: Vec<CargoDep>,
}

/// Read all manifest files from a repo path and extract dependency names.
fn extract_repo_deps(repo_name: &str, repo_path: &Path) -> RepoDeps {
    let mut all_deps = HashSet::new();
    let mut direct_refs = Vec::new();

    // Try Cargo.toml
    let cargo_path = repo_path.join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&cargo_path) {
        let (deps, cargo_directs) = parse_cargo_toml(&content);
        all_deps.extend(deps);
        direct_refs.extend(cargo_directs);
    }

    // Try package.json
    let pkg_path = repo_path.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_path) {
        all_deps.extend(parse_package_json(&content));
    }

    // Try go.mod
    let gomod_path = repo_path.join("go.mod");
    if let Ok(content) = std::fs::read_to_string(&gomod_path) {
        all_deps.extend(parse_go_mod(&content));
    }

    // Try requirements.txt
    let req_path = repo_path.join("requirements.txt");
    if let Ok(content) = std::fs::read_to_string(&req_path) {
        all_deps.extend(parse_requirements_txt(&content));
    }

    RepoDeps {
        name: repo_name.to_string(),
        deps: all_deps,
        direct_refs,
    }
}

// ---------------------------------------------------------------------------
// Coupling computation — pure functions
// ---------------------------------------------------------------------------

/// Compute shared dependencies between two repos.
fn compute_shared_deps(deps_a: &HashSet<String>, deps_b: &HashSet<String>) -> Vec<String> {
    let mut shared: Vec<String> = deps_a.intersection(deps_b).cloned().collect();
    shared.sort();
    shared
}

/// Compute dependency coupling score: shared / union.
fn compute_dep_score(
    shared_count: usize,
    deps_a: &HashSet<String>,
    deps_b: &HashSet<String>,
) -> f64 {
    let union_count = deps_a.union(deps_b).count();
    if union_count == 0 {
        return 0.0;
    }
    (shared_count as f64 / union_count as f64) * 100.0
}

/// Detect direct dependency between two repos based on Cargo.toml path/git references.
fn detect_direct_dependency(
    repo_a_name: &str,
    repo_a_refs: &[CargoDep],
    repo_b_name: &str,
    repo_b_refs: &[CargoDep],
) -> Option<DirectDep> {
    // Check if A references B
    if repo_a_refs.iter().any(|r| r.references_repo == repo_b_name) {
        return Some(DirectDep {
            from: repo_a_name.to_string(),
            to: repo_b_name.to_string(),
        });
    }

    // Check if B references A
    if repo_b_refs.iter().any(|r| r.references_repo == repo_a_name) {
        return Some(DirectDep {
            from: repo_b_name.to_string(),
            to: repo_a_name.to_string(),
        });
    }

    None
}

/// Compute blast radius: dependencies consumed by 3+ repos.
fn compute_blast_radius(repo_deps: &[RepoDeps]) -> Vec<BlastRadiusEntry> {
    let mut dep_consumers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for repo in repo_deps {
        for dep in &repo.deps {
            dep_consumers
                .entry(dep.clone())
                .or_default()
                .insert(repo.name.clone());
        }
    }

    dep_consumers
        .into_iter()
        .filter(|(_, consumers)| consumers.len() >= 3)
        .map(|(dep_name, consumers)| {
            let consumer_count = consumers.len();
            BlastRadiusEntry {
                dependency_name: dep_name,
                consumers: consumers.into_iter().collect(),
                consumer_count,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyze dependency coupling across all provided repositories.
///
/// Scans manifest files (Cargo.toml, package.json, go.mod, requirements.txt)
/// in each repo path to detect shared dependencies and direct repo-to-repo
/// dependencies.
pub fn analyze_dependency_coupling(repo_paths: &[(String, PathBuf)]) -> DependencyAnalysis {
    let repo_deps: Vec<RepoDeps> = repo_paths
        .iter()
        .map(|(name, path)| extract_repo_deps(name, path))
        .collect();

    let pairs = build_coupling_pairs(&repo_deps);
    let blast_radius = compute_blast_radius(&repo_deps);

    DependencyAnalysis {
        pairs,
        blast_radius,
    }
}

/// Build coupling pairs for all repo combinations.
fn build_coupling_pairs(repo_deps: &[RepoDeps]) -> Vec<DependencyCouplingPair> {
    let mut pairs = Vec::new();

    for i in 0..repo_deps.len() {
        for j in (i + 1)..repo_deps.len() {
            let a = &repo_deps[i];
            let b = &repo_deps[j];

            let shared_deps = compute_shared_deps(&a.deps, &b.deps);
            let shared_count = shared_deps.len();
            let dep_score = compute_dep_score(shared_count, &a.deps, &b.deps);
            let direct_dependency =
                detect_direct_dependency(&a.name, &a.direct_refs, &b.name, &b.direct_refs);

            // Include pair if there are shared deps or a direct dependency
            if shared_count > 0 || direct_dependency.is_some() {
                pairs.push(DependencyCouplingPair {
                    repo_a: a.name.clone(),
                    repo_b: b.name.clone(),
                    shared_deps,
                    shared_count,
                    dep_score,
                    direct_dependency,
                });
            }
        }
    }

    // Sort by dep_score descending
    pairs.sort_by(|a, b| {
        b.dep_score
            .partial_cmp(&a.dep_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cargo_toml_extracts_dependency_names() {
        let content = r#"
[package]
name = "my-crate"

[dependencies]
serde = "1"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
assert_cmd = "2"
"#;
        let (deps, directs) = parse_cargo_toml(content);
        assert!(deps.contains(&"serde".to_string()));
        assert!(deps.contains(&"tokio".to_string()));
        assert!(deps.contains(&"assert_cmd".to_string()));
        assert!(directs.is_empty());
    }

    #[test]
    fn parse_cargo_toml_detects_path_dependency() {
        let content = r#"
[dependencies]
other-crate = { path = "../other-crate" }
"#;
        let (deps, directs) = parse_cargo_toml(content);
        assert!(deps.is_empty());
        assert_eq!(directs.len(), 1);
        assert_eq!(directs[0].references_repo, "other-crate");
    }

    #[test]
    fn parse_cargo_toml_detects_git_dependency() {
        let content = r#"
[dependencies]
my-lib = { git = "https://github.com/org/my-lib.git" }
"#;
        let (deps, directs) = parse_cargo_toml(content);
        assert!(deps.is_empty());
        assert_eq!(directs.len(), 1);
        assert_eq!(directs[0].references_repo, "my-lib");
    }

    #[test]
    fn parse_package_json_extracts_all_deps() {
        let content = r#"{
  "dependencies": { "express": "^4.0", "lodash": "^4.0" },
  "devDependencies": { "jest": "^29.0" }
}"#;
        let deps = parse_package_json(content);
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&"express".to_string()));
        assert!(deps.contains(&"lodash".to_string()));
        assert!(deps.contains(&"jest".to_string()));
    }

    #[test]
    fn parse_go_mod_extracts_require_block() {
        let content = "module example.com/foo\n\nrequire (\n\tgithub.com/gin v1.0\n\tgithub.com/redis v2.0\n)\n";
        let deps = parse_go_mod(content);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"github.com/gin".to_string()));
        assert!(deps.contains(&"github.com/redis".to_string()));
    }

    #[test]
    fn parse_requirements_txt_extracts_package_names() {
        let content = "flask==2.3.0\nrequests>=2.28\nnumpy\n# comment\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&"flask".to_string()));
        assert!(deps.contains(&"requests".to_string()));
        assert!(deps.contains(&"numpy".to_string()));
    }

    #[test]
    fn compute_shared_deps_finds_intersection() {
        let a: HashSet<String> = ["serde", "tokio", "anyhow"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["serde", "tokio", "clap"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let shared = compute_shared_deps(&a, &b);
        assert_eq!(shared, vec!["serde", "tokio"]);
    }

    #[test]
    fn compute_dep_score_with_overlap() {
        let a: HashSet<String> = ["serde", "tokio"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["serde", "clap"].iter().map(|s| s.to_string()).collect();
        // shared=1 (serde), union=3 (serde, tokio, clap) => 33.33%
        let score = compute_dep_score(1, &a, &b);
        assert!((score - 33.333).abs() < 0.1);
    }

    #[test]
    fn blast_radius_only_includes_3_plus_consumers() {
        let repos = vec![
            RepoDeps {
                name: "a".to_string(),
                deps: ["x", "y"].iter().map(|s| s.to_string()).collect(),
                direct_refs: Vec::new(),
            },
            RepoDeps {
                name: "b".to_string(),
                deps: ["x", "y"].iter().map(|s| s.to_string()).collect(),
                direct_refs: Vec::new(),
            },
            RepoDeps {
                name: "c".to_string(),
                deps: ["x", "z"].iter().map(|s| s.to_string()).collect(),
                direct_refs: Vec::new(),
            },
        ];

        let blast = compute_blast_radius(&repos);
        // x has 3 consumers, y has 2
        assert_eq!(blast.len(), 1);
        assert_eq!(blast[0].dependency_name, "x");
        assert_eq!(blast[0].consumer_count, 3);
    }
}
