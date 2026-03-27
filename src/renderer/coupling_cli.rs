use crate::coupling::temporal::TemporalCouplingPair;
use std::fmt::Write;

/// Render a ranked CLI table of temporal coupling pairs.
///
/// Columns: Rank, Repo A, Repo B, Score, Co-Changes, Confidence
/// Pairs should already be sorted by temporal_score descending.
pub fn render_coupling_table(pairs: &[TemporalCouplingPair]) -> String {
    let headers = [
        "Rank",
        "Repo A",
        "Repo B",
        "Score",
        "Co-Changes",
        "Confidence",
    ];

    // Compute column widths from headers and data
    let widths = compute_column_widths(pairs, &headers);

    let mut output = String::new();

    // Header row
    write_row(&mut output, &headers, &widths);
    write_separator(&mut output, &widths);

    // Data rows
    for (rank, pair) in pairs.iter().enumerate() {
        let rank_str = (rank + 1).to_string();
        let score_str = format!("{:.1}", pair.temporal_score);
        let co_changes_str = pair.co_changes.to_string();
        let confidence_str = pair.confidence.to_string();

        let cells = [
            rank_str.as_str(),
            &pair.repo_a,
            &pair.repo_b,
            &score_str,
            &co_changes_str,
            &confidence_str,
        ];
        write_row(&mut output, &cells, &widths);
    }

    output
}

/// Compute the width of each column based on headers and data.
fn compute_column_widths(pairs: &[TemporalCouplingPair], headers: &[&str]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

    for (i, pair) in pairs.iter().enumerate() {
        widths[0] = widths[0].max((i + 1).to_string().len());
        widths[1] = widths[1].max(pair.repo_a.len());
        widths[2] = widths[2].max(pair.repo_b.len());
        widths[3] = widths[3].max(format!("{:.1}", pair.temporal_score).len());
        widths[4] = widths[4].max(pair.co_changes.to_string().len());
        widths[5] = widths[5].max(pair.confidence.to_string().len());
    }

    widths
}

/// Write a single row of cells padded to column widths.
fn write_row(output: &mut String, cells: &[&str], widths: &[usize]) {
    for (i, (cell, &width)) in cells.iter().zip(widths.iter()).enumerate() {
        if i > 0 {
            output.push_str("  ");
        }
        let _ = write!(output, "{:<width$}", cell, width = width);
    }
    output.push('\n');
}

/// Write a separator line matching column widths.
fn write_separator(output: &mut String, widths: &[usize]) {
    for (i, &width) in widths.iter().enumerate() {
        if i > 0 {
            output.push_str("  ");
        }
        for _ in 0..width {
            output.push('-');
        }
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coupling::temporal::Confidence;

    fn make_pair(
        repo_a: &str,
        repo_b: &str,
        score: f64,
        co_changes: usize,
        confidence: Confidence,
    ) -> TemporalCouplingPair {
        TemporalCouplingPair {
            repo_a: repo_a.to_string(),
            repo_b: repo_b.to_string(),
            co_changes,
            temporal_score: score,
            confidence,
        }
    }

    #[test]
    fn render_empty_pairs_returns_header_only() {
        let output = render_coupling_table(&[]);
        assert!(
            output.contains("Score"),
            "should contain header even when empty"
        );
        assert!(
            output.contains("Confidence"),
            "should contain Confidence header"
        );
    }

    #[test]
    fn render_includes_repo_names_and_rank() {
        let pairs = vec![
            make_pair("alpha", "beta", 80.0, 15, Confidence::Medium),
            make_pair("gamma", "delta", 50.0, 5, Confidence::Low),
        ];
        let output = render_coupling_table(&pairs);
        assert!(output.contains("alpha"));
        assert!(output.contains("beta"));
        assert!(output.contains("gamma"));
        assert!(output.contains("delta"));
        // First pair (higher score) should have rank 1
        assert!(output.contains("1"));
        assert!(output.contains("2"));
    }

    #[test]
    fn render_displays_score_and_co_changes() {
        let pairs = vec![make_pair("a", "b", 75.5, 20, Confidence::Medium)];
        let output = render_coupling_table(&pairs);
        assert!(output.contains("75.5"), "should display score");
        assert!(output.contains("20"), "should display co-changes count");
        assert!(output.contains("MEDIUM"), "should display confidence level");
    }
}
