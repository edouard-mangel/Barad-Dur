/// Select a representative sample of commits from a chronologically-ordered list.
///
/// Pure function — no I/O, no side effects.
///
/// - When `count >= len`, returns all commits.
/// - When `count == 0`, returns empty.
/// - When `count == 1`, returns the first (newest) commit.
/// - Otherwise uses index formula `i * (len - 1) / (count - 1)` for uniform spacing.
pub fn select_samples(commits: &[String], count: usize) -> Vec<String> {
    let len = commits.len();
    if count == 0 || len == 0 {
        return Vec::new();
    }
    if count >= len {
        return commits.to_vec();
    }
    if count == 1 {
        return vec![commits[0].clone()];
    }
    (0..count)
        .map(|i| {
            let index = i * (len - 1) / (count - 1);
            commits[index].clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commits(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("sha{:04}", i)).collect()
    }

    #[test]
    fn select_samples_fifteen_commits_count_ten_returns_ten() {
        let commits = make_commits(15);
        let result = select_samples(&commits, 10);
        assert_eq!(result.len(), 10, "should return exactly 10 samples");
    }

    #[test]
    fn select_samples_five_commits_count_ten_returns_all_five() {
        let commits = make_commits(5);
        let result = select_samples(&commits, 10);
        assert_eq!(result.len(), 5, "count >= len should return all commits");
        assert_eq!(result, commits);
    }

    #[test]
    fn select_samples_empty_vec_returns_empty() {
        let commits: Vec<String> = Vec::new();
        let result = select_samples(&commits, 10);
        assert!(result.is_empty(), "empty input should produce empty output");
    }

    #[test]
    fn select_samples_count_zero_returns_empty() {
        let commits = make_commits(15);
        let result = select_samples(&commits, 0);
        assert!(result.is_empty(), "count=0 should produce empty output");
    }
}
