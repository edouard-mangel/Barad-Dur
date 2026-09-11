use serde::Serialize;

/// Repo-level day-bucketed churn shape (Crime Scene Ch. 14, trends M1).
/// `None` on the report = no non-merge commits in the window.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct ChurnTimelineReport {
    /// Bucket width in days (always 1 in v1; field future-proofs wider buckets).
    pub bucket_days: u32,
    pub merge_commits_excluded: bool,
    /// One entry per UTC day from first to last active day, zero-filled.
    pub buckets: Vec<ChurnBucket>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct ChurnBucket {
    /// UTC day, `YYYY-MM-DD`.
    pub date: String,
    pub added: u64,
    pub deleted: u64,
}
