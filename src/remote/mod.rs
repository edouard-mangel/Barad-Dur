pub mod clone;
pub mod github;

/// Returns true if the target string looks like a remote URL rather than a local path.
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("git@")
}
