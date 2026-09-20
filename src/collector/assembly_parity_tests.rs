//! Characterization of the two snapshot assembly paths before they share
//! code (M03 task 1).
//!
//! The working-tree path (`collect_snapshot_with_options`) reads files from
//! disk in parallel and manifests from disk; the historical path
//! (`collect_snapshot_at_with_ast`) reads blobs at a commit, sequentially,
//! and manifests from that commit's tree. For the same committed content
//! every analysis channel must agree; where the two paths differ on purpose
//! — blame, provenance of manifests, unreadable files — the difference is
//! the assertion. Ordering and index relationships are pinned so a shared
//! implementation cannot drift into nondeterministic container traversal.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::snapshot::{BaseRef, CalleeRef, CouplingKind, ReExportKind, RepoSnapshot, TimeWindow};

use super::ignore_file::BaradDurIgnore;
use super::testutil::{git, head, repository, snapshot_paths as paths, write_tree};
use super::{Collector, SnapshotOptions};

/// Every language the collector treats differently, in one tree:
/// - Rust: a Common finding, a same-file call, a `crate::` import;
/// - TS: a barrel with star and named re-exports, a class extending an
///   imported class, calls to a same-file helper and an external package,
///   two imports of one file;
/// - PHP: PSR-4 roots from a nested `composer.json`;
/// - Go: a resolver known to be unreliable (counts specifiers, no edges);
/// - a non-UTF-8 blob, an image, and a default-excluded lockfile.
const TREE: &[(&str, &[u8])] = &[
    (
        "src/lib.rs",
        b"static mut CACHE: usize = 0;\npub mod util;\nuse crate::util::helper;\npub fn f() { helper(); }\n",
    ),
    ("src/util.rs", b"pub fn helper() {}\n"),
    (
        "web/a/index.ts",
        b"export * from './impl';\nexport { Impl as Alias } from './impl';\n",
    ),
    ("web/a/impl.ts", b"export class Impl {}\n"),
    (
        "web/b/user.ts",
        b"import { Impl } from '../a/impl';\nimport type { Impl as T } from '../a/impl';\nimport { g } from 'lodash';\nfunction helper() {}\nexport class User extends Impl {\n  m() { helper(); g(); }\n}\n",
    ),
    (
        "api/composer.json",
        b"{\"autoload\":{\"psr-4\":{\"App\\\\\":\"app/\"}}}\n",
    ),
    (
        "api/app/Foo.php",
        b"<?php\nnamespace App;\nuse App\\Bar;\nclass Foo { public function run(Bar $b) {} }\n",
    ),
    ("api/app/Bar.php", b"<?php\nnamespace App;\nclass Bar {}\n"),
    (
        "cmd/x.go",
        b"package main\nimport \"fmt\"\nfunc main() { fmt.Println() }\n",
    ),
    ("data/blob.bin", &[0xff, 0xfe, 0x9f, 0x00, 0x01]),
    ("assets/logo.png", &[0x89, b'P', b'N', b'G', 0x0d, 0x0a]),
    ("Cargo.lock", b"# lock\n"),
];

fn live(dir: &Path, skip_blame: bool) -> RepoSnapshot {
    Collector::open(dir, TimeWindow::full_history())
        .unwrap()
        .collect_snapshot_with_options(&SnapshotOptions {
            skip_blame,
            no_cache: true,
            ..SnapshotOptions::default()
        })
        .unwrap()
}

fn historical(dir: &Path, sha: &str) -> RepoSnapshot {
    let ignore = BaradDurIgnore::load(dir).unwrap();
    Collector::collect_snapshot_at_with_ast(dir, sha, &ignore, true).unwrap()
}

fn sorted_graph(snapshot: &RepoSnapshot) -> Vec<(String, Vec<String>)> {
    let mut edges: Vec<(String, Vec<String>)> = snapshot
        .import_graph
        .iter()
        .map(|(from, to)| {
            let mut to: Vec<String> = to.iter().map(|p| p.display().to_string()).collect();
            to.sort();
            (from.display().to_string(), to)
        })
        .collect();
    edges.sort();
    edges
}

/// The analysis channels, in a comparable shape: everything the AST pass
/// contributes to a snapshot, independent of acquisition metadata.
fn channels(snapshot: &RepoSnapshot) -> serde_json::Value {
    let mut metrics: Vec<(&PathBuf, &crate::snapshot::FileComplexity)> =
        snapshot.file_metrics.iter().collect();
    metrics.sort_by_key(|(path, _)| (*path).clone());
    serde_json::json!({
        "files": paths(snapshot),
        "file_metrics": metrics,
        "import_graph": sorted_graph(snapshot),
        "unreliable_import_specifiers": snapshot.unreliable_import_specifiers,
        "coupling_findings": snapshot.coupling_findings,
        "class_records": snapshot.class_records,
        "reexports": snapshot.reexports,
        "call_records": snapshot.call_records,
    })
}

// ---------------------------------------------------------------------------
// Parity: same committed content, both readers, every channel
// ---------------------------------------------------------------------------

#[test]
fn live_and_historical_agree_on_every_analysis_channel_for_committed_content() {
    let (dir, sha) = repository(TREE);
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    assert_eq!(live.head_commit, sha);
    assert_eq!(historical.head_commit, sha);
    assert_eq!(channels(&live), channels(&historical));
}

#[test]
fn the_fixture_exercises_every_channel() {
    // A parity assertion over empty channels proves nothing; pin that each
    // one carries what the tree was written to produce.
    let (dir, sha) = repository(TREE);
    let snapshot = historical(dir.path(), &sha);

    let files = paths(&snapshot);
    assert!(
        files.contains(&"data/blob.bin".into()),
        "binary files stay listed"
    );
    assert!(
        !files.contains(&"Cargo.lock".into()),
        "default exclusion applies"
    );

    assert!(snapshot.file_metrics.contains_key(Path::new("src/lib.rs")));
    assert!(snapshot
        .file_metrics
        .contains_key(Path::new("web/b/user.ts")));
    assert!(
        !snapshot
            .file_metrics
            .contains_key(Path::new("data/blob.bin")),
        "flagged binary by libgit2 (UTF-16 BOM): never parsed"
    );
    assert!(
        !snapshot
            .file_metrics
            .contains_key(Path::new("assets/logo.png")),
        "not flagged binary, but 0x89 is not UTF-8: skipped, not measured"
    );

    let graph = sorted_graph(&snapshot);
    let edges_of = |from: &str| -> Vec<String> {
        graph
            .iter()
            .find(|(f, _)| f == from)
            .map(|(_, to)| to.clone())
            .unwrap_or_default()
    };
    assert_eq!(
        edges_of("web/b/user.ts"),
        ["web/a/impl.ts"],
        "two imports of one file yield one edge"
    );
    assert_eq!(
        edges_of("web/a/index.ts"),
        Vec::<String>::new(),
        "re-exports are their own channel, not import edges"
    );
    assert_eq!(
        edges_of("src/lib.rs"),
        ["src/util.rs"],
        "a Rust symbol import (`use crate::util::helper`) resolves to the module \
         that declares the symbol, like the call channel below"
    );
    assert_eq!(
        edges_of("api/app/Foo.php"),
        ["api/app/Bar.php"],
        "PSR-4 root from api/composer.json resolves the PHP import"
    );
    assert_eq!(edges_of("cmd/x.go"), Vec::<String>::new());
    assert_eq!(
        snapshot.unreliable_import_specifiers, 1,
        "the Go import is counted as unreliable, not resolved"
    );

    let kinds: Vec<(String, CouplingKind)> = snapshot
        .coupling_findings
        .iter()
        .map(|f| (f.path.display().to_string(), f.kind))
        .collect();
    assert_eq!(kinds, [("src/lib.rs".to_string(), CouplingKind::Common)]);

    assert_eq!(
        snapshot.class_records.len(),
        1,
        "only inheritance sites are recorded; a class without `extends` is not"
    );
    let user = snapshot
        .class_records
        .iter()
        .find(|c| c.class_name == "User")
        .expect("User class record");
    assert_eq!(
        user.base,
        BaseRef::Resolved {
            path: "web/a/impl.ts".into(),
            name: "Impl".into()
        }
    );

    let reexport_kinds: Vec<(&str, bool)> = snapshot
        .reexports
        .iter()
        .map(|r| {
            (
                r.target.to_str().unwrap(),
                matches!(r.kind, ReExportKind::Star),
            )
        })
        .collect();
    assert_eq!(
        reexport_kinds,
        [("web/a/impl.ts", true), ("web/a/impl.ts", false)],
        "both resolve to the barrel's target; equal sort keys (path, target) \
         keep extraction order — star first, as written — so the sort must \
         stay stable"
    );

    let calls: Vec<(String, String, String)> = snapshot
        .call_records
        .iter()
        .map(|c| {
            let callee = match &c.callee {
                CalleeRef::SameFile(n) => format!("same:{n}"),
                CalleeRef::Resolved { path, name } => format!("{}:{name}", path.display()),
                CalleeRef::Unresolved { name } => format!("unresolved:{name}"),
            };
            (c.path.display().to_string(), c.caller.clone(), callee)
        })
        .collect();
    assert!(
        calls.contains(&("src/lib.rs".into(), "f".into(), "src/util.rs:helper".into())),
        "the Rust call channel resolves the imported symbol to its file: {calls:?}"
    );
    assert!(
        calls.contains(&("web/b/user.ts".into(), "m".into(), "same:helper".into())),
        "{calls:?}"
    );
    assert!(
        calls.contains(&("web/b/user.ts".into(), "m".into(), "unresolved:g".into())),
        "an external package call stays countable as unresolved: {calls:?}"
    );
}

// ---------------------------------------------------------------------------
// Ordering and indexes
// ---------------------------------------------------------------------------

fn assert_sorted<T: Ord + std::fmt::Debug>(keys: Vec<T>, what: &str) {
    let mut sorted: Vec<&T> = keys.iter().collect();
    sorted.sort();
    assert_eq!(
        keys.iter().collect::<Vec<_>>(),
        sorted,
        "{what} must be emitted in sorted order"
    );
}

#[test]
fn resolved_channels_are_deterministically_ordered_on_both_paths() {
    let (dir, sha) = repository(TREE);
    for snapshot in [live(dir.path(), true), historical(dir.path(), &sha)] {
        assert_sorted(
            snapshot
                .coupling_findings
                .iter()
                .map(|f| (f.path.clone(), f.line))
                .collect(),
            "coupling findings (path, line)",
        );
        assert_sorted(
            snapshot
                .class_records
                .iter()
                .map(|c| (c.path.clone(), c.line))
                .collect(),
            "class records (path, line)",
        );
        assert_sorted(
            snapshot
                .reexports
                .iter()
                .map(|r| (r.path.clone(), r.target.clone()))
                .collect(),
            "re-exports (path, target)",
        );
        assert_sorted(
            snapshot
                .call_records
                .iter()
                .map(|c| (c.path.clone(), c.caller.clone(), c.callee.clone()))
                .collect(),
            "call records (path, caller, callee)",
        );
        assert_sorted(paths(&snapshot), "file entries");
        // Co-change pair ordering is not observable here: the fixture has one
        // commit and the pair index keeps pairs seen at least three times, so
        // `file_change_pairs` is empty on both paths. The descending sort is
        // pinned by `assemble_builds_indexes_once_from_final_core_data_and_keeps_provenance`.
        assert!(snapshot.file_change_pairs.is_empty());
    }
}

#[test]
fn indexes_reference_only_final_core_data_on_both_paths() {
    let (dir, sha) = repository(TREE);
    for snapshot in [live(dir.path(), true), historical(dir.path(), &sha)] {
        let commit_ids: std::collections::HashSet<_> =
            snapshot.commits.iter().map(|c| c.id).collect();
        let author_ids: std::collections::HashSet<_> =
            snapshot.authors.iter().map(|a| a.id).collect();
        let known = snapshot.known_paths();
        for (author, commits) in &snapshot.commits_by_author {
            assert!(author_ids.contains(author));
            assert!(commits.iter().all(|c| commit_ids.contains(c)));
        }
        for (path, commits) in &snapshot.commits_by_file {
            assert!(commits.iter().all(|c| commit_ids.contains(c)), "{path:?}");
        }
        for (a, b, _) in &snapshot.file_change_pairs {
            assert!(
                known.contains(a) && known.contains(b),
                "pairs only over listed files"
            );
        }
        assert_eq!(snapshot.commits.len(), 1);
        assert_eq!(snapshot.authors.len(), 1);
        assert_eq!(
            snapshot.resolve_commit(snapshot.commits[0].id),
            snapshot.head_commit
        );
    }
}

// ---------------------------------------------------------------------------
// Intentional differences between the two readers
// ---------------------------------------------------------------------------

#[test]
fn historical_collection_never_blames_even_where_live_does() {
    let (dir, sha) = repository(TREE);
    let live = live(dir.path(), false);
    let historical = historical(dir.path(), &sha);
    assert!(
        live.blame_map.contains_key(Path::new("src/lib.rs")),
        "live collection blames text files"
    );
    assert!(
        historical.blame_map.is_empty(),
        "ADR-005: blame is what the historical path skips, never the AST pass"
    );
    assert_eq!(
        channels(&live),
        channels(&historical),
        "blame does not change the AST channels"
    );
}

#[test]
fn live_collection_pins_every_phase_to_one_head_observation() {
    // HEAD can move while a multi-minute collection runs (a pull, a commit,
    // a checkout). Commits, files and the stamped `head_commit` must all
    // come from the head the collection observed once at its start, never
    // from whatever HEAD points at when a later phase happens to read it:
    // the cache keys on `head_commit`, so a mixed snapshot would be served
    // as fresh for a tree it does not describe.
    let (dir, first) = repository(TREE);
    let collector = Collector::open(dir.path(), TimeWindow::full_history()).unwrap();
    write_tree(dir.path(), &[("src/late.rs", b"pub fn late() {}\n")]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "late"]);
    assert_ne!(head(dir.path()), first, "HEAD moved after the observation");

    let snapshot = collector
        .collect_snapshot_inner(
            &first,
            &SnapshotOptions {
                skip_blame: true,
                no_cache: true,
                ..SnapshotOptions::default()
            },
        )
        .unwrap();
    assert_eq!(snapshot.head_commit, first);
    assert_eq!(
        snapshot.commits.len(),
        1,
        "commits walked from the pinned head"
    );
    assert!(
        !paths(&snapshot).contains(&"src/late.rs".to_string()),
        "files listed from the pinned head's tree"
    );
}

#[test]
fn manifests_come_from_disk_live_and_from_the_commit_historically() {
    let (dir, sha) = repository(TREE);
    // Uncommitted edit: the working-tree manifest now maps the namespace to
    // a directory that holds nothing, so live PHP edges vanish while the
    // commit's manifest still resolves them.
    std::fs::write(
        dir.path().join("api/composer.json"),
        "{\"autoload\":{\"psr-4\":{\"App\\\\\":\"elsewhere/\"}}}\n",
    )
    .unwrap();
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    let php_edges = |s: &RepoSnapshot| s.import_graph.get(Path::new("api/app/Foo.php")).cloned();
    assert_eq!(php_edges(&live), None);
    assert_eq!(
        php_edges(&historical),
        Some(vec![PathBuf::from("api/app/Bar.php")])
    );
}

#[test]
fn source_content_comes_from_disk_live_and_from_the_commit_historically() {
    let (dir, sha) = repository(TREE);
    // Uncommitted edit removes the Common finding from the working tree.
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn f() {}\n").unwrap();
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    assert!(live.coupling_findings.is_empty());
    assert_eq!(historical.coupling_findings.len(), 1);
}

#[test]
fn the_current_ignore_file_applies_to_both_readers() {
    let (dir, sha) = repository(TREE);
    // Uncommitted rule: current policy filters historical snapshots too,
    // so a trend reflects one definition of "relevant files".
    std::fs::write(dir.path().join(".baraddurignore"), "web/\n").unwrap();
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    for snapshot in [&live, &historical] {
        assert!(paths(snapshot).iter().all(|p| !p.starts_with("web/")));
        assert!(snapshot.class_records.is_empty());
        assert!(snapshot.reexports.is_empty());
    }
    assert_eq!(channels(&live), channels(&historical));
}

#[test]
fn an_unreadable_working_tree_file_is_skipped_live_but_read_from_its_blob() {
    let (dir, sha) = repository(TREE);
    // Deleting the file, rather than chmod'ing it, makes the read fail for
    // every uid (root ignores mode bits) and on every platform: the file
    // list comes from HEAD's tree, so the entry stays listed, and its blob
    // is still in the object database for the historical reader.
    std::fs::remove_file(dir.path().join("src/lib.rs")).unwrap();
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    assert!(
        live.files.iter().any(|f| f.path == Path::new("src/lib.rs")),
        "the entry is listed from HEAD's tree even though the file is gone"
    );
    assert!(
        !live.file_metrics.contains_key(Path::new("src/lib.rs")),
        "an unreadable working-tree file is skipped, not analysed as empty"
    );
    assert!(historical
        .file_metrics
        .contains_key(Path::new("src/lib.rs")));
}

// ---------------------------------------------------------------------------
// Edge shapes
// ---------------------------------------------------------------------------

#[test]
fn an_empty_tree_yields_empty_channels_and_no_detection_on_both_paths() {
    let dir = tempfile::TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(
        dir.path(),
        &["commit", "-q", "--allow-empty", "-m", "empty"],
    );
    let sha = head(dir.path());
    for snapshot in [live(dir.path(), true), historical(dir.path(), &sha)] {
        assert!(snapshot.files.is_empty());
        assert!(snapshot.file_metrics.is_empty());
        assert!(snapshot.import_graph.is_empty());
        assert_eq!(snapshot.unreliable_import_specifiers, 0);
        assert!(snapshot.coupling_findings.is_empty());
        assert!(snapshot.class_records.is_empty());
        assert!(snapshot.reexports.is_empty());
        assert!(snapshot.call_records.is_empty());
        assert!(snapshot.file_change_pairs.is_empty());
        assert_eq!(snapshot.commits.len(), 1);
    }
}

#[test]
fn a_binary_flagged_blob_is_never_parsed_even_when_it_decodes_as_utf8() {
    // The two fixture "binaries" are skipped for different reasons: the
    // UTF-16 BOM makes libgit2 flag `data/blob.bin` binary, while
    // `assets/logo.png` is short enough to pass libgit2's heuristic and is
    // dropped only because 0x89 is not valid UTF-8. Neither pins the
    // `!is_binary` filter itself. A NUL byte inside otherwise plain text
    // does: libgit2 flags it binary, `from_utf8` accepts it, so only the
    // filter keeps it out of the metrics — on both readers.
    let mut tree = TREE.to_vec();
    tree.push(("src/nul.rs", b"pub fn g() {}\0\n"));
    let (dir, sha) = repository(&tree);
    for snapshot in [live(dir.path(), true), historical(dir.path(), &sha)] {
        let flag = |name: &str| {
            snapshot
                .files
                .iter()
                .find(|f| f.path == Path::new(name))
                .unwrap_or_else(|| panic!("{name} listed"))
                .is_binary
        };
        assert!(flag("src/nul.rs"), "NUL byte: flagged binary by libgit2");
        assert!(
            flag("data/blob.bin"),
            "UTF-16 BOM: flagged binary by libgit2"
        );
        assert!(
            !flag("assets/logo.png"),
            "six printable-ish bytes: NOT flagged binary, skipped by from_utf8 instead"
        );
        assert!(
            !snapshot.file_metrics.contains_key(Path::new("src/nul.rs")),
            "a binary-flagged entry is never parsed, even though it decodes as UTF-8"
        );
    }
}

#[test]
fn an_unresolvable_import_leaves_no_edge_but_keeps_the_file_measured() {
    let (dir, sha) = repository(&[(
        "web/only.ts",
        b"import { x } from './missing';\nexport const y = x;\n",
    )]);
    for snapshot in [live(dir.path(), true), historical(dir.path(), &sha)] {
        assert!(snapshot.file_metrics.contains_key(Path::new("web/only.ts")));
        assert!(
            !snapshot.import_graph.contains_key(Path::new("web/only.ts")),
            "an import that resolves to nothing contributes no edge"
        );
        assert_eq!(snapshot.unreliable_import_specifiers, 0);
    }
}

#[test]
fn ast_free_historical_collection_is_an_explicit_mode_not_a_measured_absence() {
    let (dir, sha) = repository(TREE);
    let ignore = BaradDurIgnore::load(dir.path()).unwrap();
    let without = Collector::collect_snapshot_at(dir.path(), &sha, &ignore, true).unwrap();
    let with = historical(dir.path(), &sha);
    assert_eq!(paths(&without), paths(&with), "same tree, same exclusions");
    assert!(without.file_metrics.is_empty());
    assert!(without.import_graph.is_empty());
    assert_eq!(without.unreliable_import_specifiers, 0);
    assert!(without.coupling_findings.is_empty());
    assert!(
        !crate::metrics::coupling::detection_ran(&without),
        "empty metrics read as not collected, never as clean"
    );
    assert!(crate::metrics::coupling::detection_ran(&with));
}

#[test]
fn provenance_fields_follow_the_reader() {
    let (dir, sha) = repository(TREE);
    // The name comes from the origin remote when there is one, on both
    // readers: a gate baseline or a backfill entry must name the repository
    // the way `analyze` does, whatever directory it was cloned into.
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://example.test/team/acme.git",
        ],
    );
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    for snapshot in [&live, &historical] {
        assert_eq!(snapshot.name, "acme");
        assert_eq!(snapshot.default_branch, "main");
        let window = TimeWindow::full_history();
        assert_eq!(snapshot.time_window.since, window.since);
        assert_eq!(snapshot.time_window.until, window.until);
        assert_eq!(snapshot.time_window.default_months, window.default_months);
        // The live path is what libgit2 discovered (realpath'd); the
        // historical one is the caller's. Compare canonical forms so a
        // symlinked temp directory (macOS `/var` -> `/private/var`) does not
        // fail the live iteration alone.
        assert_eq!(
            snapshot.path.canonicalize().unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }
    // `created_at` stamping is pinned in `assemble`'s unit test; ordering the
    // two readers' wall-clock stamps here would only make this test flaky.
    let _: HashMap<PathBuf, Vec<PathBuf>> = live.import_graph;
}

#[test]
fn live_and_historical_ast_preserve_function_test_context() {
    let tree: &[(&str, &[u8])] = &[
        ("src/lib.rs", b"fn render_prod() {} #[test] fn render_case() { fn render_helper() {} }"),
        ("src/mixed.py", b"import unittest as u\nclass Checks(u.TestCase):\n def render_helper(self): pass\ndef render_prod(): pass\n"),
        ("src/mixed.ts", b"import {test as check} from 'vitest'; check('case', () => { function render_helper() {} }); function render_prod() {}"),
        ("src/mixed.java", b"import org.junit.Test; class Mixed { @Test void renderCase() {} void renderProd() {} }"),
        ("src/mixed.cs", b"using Xunit; class Mixed { [Fact] void RenderCase() {} void RenderProd() {} }"),
        ("src/mixed.kt", b"import kotlin.test.Test\n@Test fun renderCase() {}\nfun renderProd() {}\n"),
        ("src/mixed.php", b"<?php use PHPUnit\\Framework\\Attributes\\Test; class Fixture { #[Test] function renderCase() {} function renderProd() {} }"),
        ("src/mixed_test.go", b"package sample\nfunc RenderHelper() {}\n"),
        ("src/mixed.go", b"package sample\nfunc TestRender() {}\n"),
    ];
    let (dir, sha) = repository(tree);
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    assert_eq!(live.file_metrics, historical.file_metrics);
    crate::cache::save(&live, dir.path()).unwrap();
    let cached = crate::cache::load(dir.path()).unwrap().unwrap();
    assert_eq!(live.file_metrics, cached.file_metrics);
    for (path, content) in tree {
        let standalone = crate::metrics::complexity::analyse_file(
            Path::new(path),
            std::str::from_utf8(content).unwrap(),
        );
        let metrics = &live.file_metrics[Path::new(path)];
        assert_eq!(metrics, &standalone, "{path}");
        assert!(!metrics.functions.is_empty(), "{path}");
        if *path == "src/mixed.go" {
            assert!(metrics.functions.iter().all(|f| !f.is_test));
        } else {
            assert!(metrics.functions.iter().any(|f| f.is_test), "{path}");
        }
        if *path != "src/mixed_test.go" {
            assert!(metrics.functions.iter().any(|f| !f.is_test), "{path}");
        }
    }
}

#[test]
fn responsibility_provenance_matches_live_historical_standalone_and_cache() {
    let source = b"struct State { ready: bool } impl State { fn check(&self) -> bool { true } fn is_ready(&self) -> bool { self.ready && self.check() } fn is_active(&self) -> bool { self.ready } }";
    let (dir, sha) = repository(&[("src/lib.rs", source)]);
    let live = live(dir.path(), true);
    let historical = historical(dir.path(), &sha);
    let standalone = crate::metrics::complexity::analyse_file(
        Path::new("src/lib.rs"),
        std::str::from_utf8(source).unwrap(),
    );
    assert_eq!(live.file_metrics, historical.file_metrics);
    assert_eq!(live.file_metrics[Path::new("src/lib.rs")], standalone);
    let functions = &standalone.functions;
    assert_eq!(functions.len(), 3);
    let ready = functions
        .iter()
        .find(|f| f.name == "is_ready")
        .unwrap()
        .responsibility
        .as_ref()
        .unwrap();
    let active = functions
        .iter()
        .find(|f| f.name == "is_active")
        .unwrap()
        .responsibility
        .as_ref()
        .unwrap();
    assert_eq!(ready.owner_id, active.owner_id);
    assert!(ready
        .dependencies
        .iter()
        .any(|d| active.dependencies.contains(d)));
    crate::cache::save(&live, dir.path()).unwrap();
    assert_eq!(
        crate::cache::load(dir.path())
            .unwrap()
            .unwrap()
            .file_metrics,
        live.file_metrics
    );
}
