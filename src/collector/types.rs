use crate::snapshot::{Author, AuthorId, Commit, CommitInterner};
use std::collections::HashMap;

/// Result of collecting commits — includes deduplicated author list.
#[non_exhaustive]
pub struct CommitCollection {
    pub commits: Vec<Commit>,
    pub authors: Vec<Author>,
    pub interner: CommitInterner,
    /// Maps raw (pre-mailmap) email → AuthorId for blame resolution.
    ///
    /// Populated only for authors whose git-recorded email differs from the
    /// mailmap-resolved canonical email; empty when the repo has no `.mailmap`
    /// or no aliases are in use. Keys are lowercase, matching the normalisation
    /// applied to `Author::email`.
    pub raw_email_to_id: HashMap<String, AuthorId>,
}
