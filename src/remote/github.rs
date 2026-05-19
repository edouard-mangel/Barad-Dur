use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Metadata fetched from the GitHub REST API.
pub struct GitHubMeta {
    pub stars: u64,
    pub description: Option<String>,
    pub language: Option<String>,
    pub visibility: String,
    pub open_issues: u64,
}

#[derive(Deserialize)]
struct ApiResponse {
    stargazers_count: u64,
    description: Option<String>,
    language: Option<String>,
    visibility: String,
    open_issues_count: u64,
}

/// Returns true if `url` points to a GitHub repository.
pub fn is_github_url(url: &str) -> bool {
    url.contains("github.com")
}

/// Parse `owner` and `repo` from a GitHub URL.
///
/// Supports:
/// - `https://github.com/owner/repo`
/// - `https://github.com/owner/repo.git`
/// - `git@github.com:owner/repo.git`
fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let rest = rest.trim_end_matches(".git");
        let mut parts = rest.splitn(2, '/');
        let owner = parts.next()?.to_string();
        let repo = parts.next()?.to_string();
        return Some((owner, repo));
    }

    // HTTPS: https://github.com/owner/repo[.git]
    let rest = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let rest = rest.trim_start_matches("github.com/");
    let rest = rest.trim_end_matches(".git");
    let mut parts = rest.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

/// Fetch repository metadata from the GitHub API.
///
/// Requires a valid personal access token with at least `public_repo` scope
/// (or `repo` for private repositories).
pub fn fetch_meta(url: &str, token: &str) -> Result<GitHubMeta> {
    let (owner, repo) =
        parse_owner_repo(url).with_context(|| format!("Cannot parse GitHub URL: {}", url))?;

    let api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    let client = reqwest::blocking::Client::builder()
        .user_agent("barad-dur/0.1")
        .timeout(std::time::Duration::from_secs(
            crate::registry::client::TIMEOUT_SECS,
        ))
        .build()?;

    let response = client
        .get(&api_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .send()
        .with_context(|| format!("Failed to reach GitHub API for {}/{}", owner, repo))?;

    if !response.status().is_success() {
        bail!(
            "GitHub API returned {} for {}/{}",
            response.status(),
            owner,
            repo
        );
    }

    let api: ApiResponse = response
        .json()
        .context("Failed to parse GitHub API response")?;

    Ok(GitHubMeta {
        stars: api.stargazers_count,
        description: api.description,
        language: api.language,
        visibility: api.visibility,
        open_issues: api.open_issues_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_https_url() {
        let (owner, repo) = parse_owner_repo("https://github.com/BurntSushi/ripgrep").unwrap();
        assert_eq!(owner, "BurntSushi");
        assert_eq!(repo, "ripgrep");
    }

    #[test]
    fn parse_https_url_with_git_suffix() {
        let (owner, repo) = parse_owner_repo("https://github.com/BurntSushi/ripgrep.git").unwrap();
        assert_eq!(owner, "BurntSushi");
        assert_eq!(repo, "ripgrep");
    }

    #[test]
    fn parse_ssh_url() {
        let (owner, repo) = parse_owner_repo("git@github.com:BurntSushi/ripgrep.git").unwrap();
        assert_eq!(owner, "BurntSushi");
        assert_eq!(repo, "ripgrep");
    }

    #[test]
    fn is_github_url_positive() {
        assert!(is_github_url("https://github.com/owner/repo"));
        assert!(is_github_url("git@github.com:owner/repo.git"));
    }

    #[test]
    fn is_github_url_negative() {
        assert!(!is_github_url("https://gitlab.com/owner/repo"));
        assert!(!is_github_url("/local/path"));
    }
}
