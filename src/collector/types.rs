use crate::snapshot::{Author, Commit, CommitInterner};

/// Result of collecting commits — includes deduplicated author list.
#[non_exhaustive]
pub struct CommitCollection {
    pub commits: Vec<Commit>,
    pub authors: Vec<Author>,
    pub interner: CommitInterner,
}
