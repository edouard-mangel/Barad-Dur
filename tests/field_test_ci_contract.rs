//! Behavioral contracts for the blocking public-corpus CI gate.

const CI: &str = include_str!("../.gitlab-ci.yml");

#[test]
fn merge_result_is_resolved_once_and_verified_by_every_consumer() {
    assert!(
        CI.contains("git rev-parse FETCH_HEAD > .merge-result-sha"),
        "the producer must persist the exact merge-result commit"
    );
    assert!(
        CI.contains("expected_merge_sha=\"$(cat .merge-result-sha)\""),
        "consumers must read the producer's immutable commit"
    );
    assert!(
        CI.contains("test \"$fetched_merge_sha\" = \"$expected_merge_sha\""),
        "consumers must fail instead of silently testing a newer merge result"
    );
    assert_eq!(
        CI.matches("- .merge-result-sha").count(),
        1,
        "the immutable commit must be handed off as a build artifact"
    );

    for job in ["test", "coverage", "self-analysis", "field-test"] {
        let marker = format!("{job}:\n");
        let section = CI.split_once(&marker).unwrap().1;
        let section = section.split_once("\n\n").unwrap().0;
        assert!(
            section.contains("needs:") && section.contains("- build"),
            "{job} must receive the build's immutable merge-result artifact"
        );
    }
}

#[test]
fn corpus_directory_is_selected_from_the_checked_out_manifest_at_runtime() {
    assert!(
        CI.contains("manifest_hash=\"$(sha256sum field-test/corpus.toml"),
        "the checked-out manifest must be hashed in the field-test script"
    );
    assert!(
        CI.contains("BARAD_DUR_CORPUS_ROOT=\"$CI_PROJECT_DIR/.corpus/$manifest_hash\""),
        "different merged manifests must not share repository directories"
    );
    assert!(
        !CI.contains("files:\n        - field-test/corpus.toml"),
        "GitLab hashes cache-key files before the merge-result checkout"
    );
}
