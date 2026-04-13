pub mod clone;
pub mod github;

/// Returns true if the target string looks like a remote URL rather than a local path.
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("git@")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_url_accepts_https() {
        assert!(is_url("https://github.com/user/repo"));
    }

    #[test]
    fn is_url_accepts_http() {
        assert!(is_url("http://example.com/repo.git"));
    }

    #[test]
    fn is_url_accepts_git_ssh() {
        assert!(is_url("git@github.com:user/repo.git"));
    }

    #[test]
    fn is_url_rejects_absolute_path() {
        assert!(!is_url("/home/user/repo"));
    }

    #[test]
    fn is_url_rejects_relative_path() {
        assert!(!is_url("./repo"));
        assert!(!is_url("../repo"));
    }

    #[test]
    fn is_url_rejects_bare_name() {
        assert!(!is_url("repo"));
    }
}
