use std::path::PathBuf;

use crate::snapshot::{Author, FileEntry, FunctionMetrics, RepoSnapshot, TimeWindow};

pub fn make_snapshot() -> RepoSnapshot {
    RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    )
}

pub fn two_authors() -> Vec<Author> {
    vec![
        Author {
            id: 0,
            name: "Alice".into(),
            email: "alice@test.com".into(),
        },
        Author {
            id: 1,
            name: "Bob".into(),
            email: "bob@test.com".into(),
        },
    ]
}

pub fn make_file(name: &str) -> FileEntry {
    FileEntry {
        path: PathBuf::from(name),
        size_bytes: 100,
        is_binary: false,
        depth: 1,
        blob_oid: String::new(),
    }
}

pub fn normal_function(name: &str) -> FunctionMetrics {
    FunctionMetrics {
        name: name.to_string(),
        loc: 20,
        cyclomatic_complexity: 3,
        max_nesting_depth: 2,
    }
}
