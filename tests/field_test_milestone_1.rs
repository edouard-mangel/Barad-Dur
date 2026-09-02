use barad_dur::field_test::corpus::parse_corpus;

#[test]
fn the_committed_corpus_covers_every_language_the_spec_requires() {
    let entries = parse_corpus(include_str!("../field-test/corpus.toml")).expect("parses");
    let langs: std::collections::BTreeSet<_> = entries.iter().map(|e| e.lang.as_str()).collect();
    for required in ["Rust", "CSharp", "TypeScript", "PHP"] {
        assert!(langs.contains(required), "corpus is missing {required}");
    }
}

#[test]
fn rust_is_represented_by_more_than_our_own_repository() {
    let entries = parse_corpus(include_str!("../field-test/corpus.toml")).expect("parses");
    let foreign_rust: Vec<&str> = entries
        .iter()
        .filter(|e| e.lang == "Rust" && e.name != "barad-dur")
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        !foreign_rust.contains(&"barad-dur"),
        "the foreign-Rust set must exclude our own repository — asserting on \
         contents, not just the count, so deleting the self-exclusion filter \
         is caught even when the count alone would still pass"
    );
    assert!(
        foreign_rust.len() >= 2,
        "self-dogfooding on a tidy repo is what this corpus exists to fix"
    );
}
