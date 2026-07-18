//! Report-section builders: pure `(snapshot) → Vec<row>` functions the
//! scorer assembles into `AnalysisReport`. One submodule per report
//! concern; re-exported flat so `scorer.rs` call sites stay unchanged.

mod authors;
mod coupling;
mod files;
mod hotspots;

pub(super) use authors::{build_author_cards, build_author_ownership};
pub(super) use coupling::{
    build_coupling_pairs, build_import_cycles, build_import_edges, build_per_file_coupling,
};
pub(super) use files::build_file_ages;
pub(super) use hotspots::build_hotspots;

/// Snapshot scaffolding shared by the submodules' tests.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::snapshot::{Commit, CommitId, FileEntry};
    use chrono::Utc;
    use std::path::PathBuf;

    pub(crate) fn make_commit(id: u32, message: &str) -> Commit {
        Commit {
            id: CommitId(id),
            author: 0,
            timestamp: Utc::now(),
            message: message.to_string(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        }
    }

    pub(crate) fn make_file_entry(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            size_bytes: 100,
            is_binary: false,
            depth: 1,
            blob_oid: String::new(),
        }
    }

    pub(crate) fn make_commit_at(id: u32, ts: chrono::DateTime<Utc>) -> Commit {
        Commit {
            id: CommitId(id),
            author: 0,
            timestamp: ts,
            message: "chore: touch".to_string(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        }
    }
}
