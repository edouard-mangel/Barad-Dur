use crate::deps::Ecosystem;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct LockedDep {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
}

pub fn collect_locked_deps(repo_root: &Path) -> Vec<LockedDep> {
    let mut deps = Vec::new();
    deps.extend(parse_cargo_lock(repo_root));
    deps.extend(parse_npm_lock(repo_root));
    deps.extend(parse_pip_lock(repo_root));
    deps.extend(parse_nuget_lock(repo_root));
    deps
}

pub fn parse_cargo_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("Cargo.lock");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    let Ok(table) = content.parse::<toml::Value>() else {
        return vec![];
    };
    let Some(packages) = table.get("package").and_then(|v| v.as_array()) else {
        return vec![];
    };

    packages
        .iter()
        .filter_map(|pkg| {
            let name = pkg.get("name")?.as_str()?.to_string();
            let version = pkg.get("version")?.as_str()?.to_string();
            Some(LockedDep {
                name,
                version,
                ecosystem: Ecosystem::Cargo,
            })
        })
        .collect()
}

pub fn parse_npm_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("package-lock.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return vec![];
    };

    let Some(packages) = json.get("packages").and_then(|v| v.as_object()) else {
        return vec![];
    };

    packages
        .iter()
        .filter_map(|(key, val)| {
            let name = key.strip_prefix("node_modules/")?.to_string();
            if name.is_empty() {
                return None;
            }
            let version = val.get("version")?.as_str()?.to_string();
            Some(LockedDep {
                name,
                version,
                ecosystem: Ecosystem::Npm,
            })
        })
        .collect()
}

pub fn parse_pip_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("requirements.txt");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };

    content
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && l.contains("=="))
        .filter_map(|l| {
            let mut parts = l.splitn(2, "==");
            let name = parts.next()?.trim().to_string();
            let version = parts
                .next()?
                .trim()
                .split(|c: char| !c.is_alphanumeric() && c != '.')
                .next()?
                .to_string();
            if name.is_empty() || version.is_empty() {
                return None;
            }
            Some(LockedDep {
                name,
                version,
                ecosystem: Ecosystem::Pip,
            })
        })
        .collect()
}

pub fn parse_nuget_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("packages.lock.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return vec![];
    };

    let Some(deps_by_target) = json.get("dependencies").and_then(|v| v.as_object()) else {
        return vec![];
    };

    let mut result = Vec::new();
    for (_target, packages) in deps_by_target {
        if let Some(pkgs) = packages.as_object() {
            for (name, info) in pkgs {
                if let Some(version) = info.get("resolved").and_then(|v| v.as_str()) {
                    result.push(LockedDep {
                        name: name.clone(),
                        version: version.to_string(),
                        ecosystem: Ecosystem::Nuget,
                    });
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_cargo_lock_basic() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.lock"),
            r#"
[[package]]
name = "serde"
version = "1.0.130"

[[package]]
name = "tokio"
version = "1.20.0"
"#,
        )
        .unwrap();
        let deps = parse_cargo_lock(dir.path());
        assert_eq!(deps.len(), 2);
        assert!(deps
            .iter()
            .any(|d| d.name == "serde" && d.version == "1.0.130"));
        assert!(deps
            .iter()
            .any(|d| d.name == "tokio" && d.version == "1.20.0"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Cargo));
    }

    #[test]
    fn parse_npm_lock_basic() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package-lock.json"),
            r#"
{
  "lockfileVersion": 2,
  "packages": {
    "node_modules/lodash": { "version": "4.17.21" },
    "node_modules/react": { "version": "18.2.0" }
  }
}
"#,
        )
        .unwrap();
        let deps = parse_npm_lock(dir.path());
        assert_eq!(deps.len(), 2);
        assert!(deps
            .iter()
            .any(|d| d.name == "lodash" && d.version == "4.17.21"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Npm));
    }

    #[test]
    fn parse_pip_lock_requirements_txt() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("requirements.txt"),
            "requests==2.28.0\nflask==2.3.0\n# comment\n\npytest>=7.0\n",
        )
        .unwrap();
        let deps = parse_pip_lock(dir.path());
        assert!(deps
            .iter()
            .any(|d| d.name == "requests" && d.version == "2.28.0"));
        assert!(deps
            .iter()
            .any(|d| d.name == "flask" && d.version == "2.3.0"));
        assert!(!deps.iter().any(|d| d.name == "pytest"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Pip));
    }

    #[test]
    fn parse_nuget_lock_basic() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("packages.lock.json"),
            r#"
{
  "dependencies": {
    "net8.0": {
      "Newtonsoft.Json": { "type": "Direct", "resolved": "13.0.3" }
    }
  }
}
"#,
        )
        .unwrap();
        let deps = parse_nuget_lock(dir.path());
        assert!(deps
            .iter()
            .any(|d| d.name == "Newtonsoft.Json" && d.version == "13.0.3"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Nuget));
    }

    #[test]
    fn no_lock_files_returns_empty() {
        let dir = tempdir().unwrap();
        assert!(collect_locked_deps(dir.path()).is_empty());
    }
}
