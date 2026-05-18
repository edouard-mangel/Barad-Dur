use crate::snapshot::{Author, AuthorId, Commit, CommitInterner};
use std::collections::HashMap;

/// Result of collecting commits — includes deduplicated author list.
#[non_exhaustive]
pub struct CommitCollection {
    pub commits: Vec<Commit>,
    pub authors: Vec<Author>,
    pub interner: CommitInterner,
    /// Maps raw (pre-mailmap) email → AuthorId for blame resolution.
    pub raw_email_to_id: HashMap<String, AuthorId>,
}
