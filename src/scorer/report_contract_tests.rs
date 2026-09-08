#[test]
fn current_fixture_matches_actual_rust_serialization() {
    let actual = crate::report_contract::report_fixture_typescript().unwrap();
    let expected = include_str!("../../tests/fixtures/report-contract/current.ts");

    assert_eq!(
        actual, expected,
        "update the fixture explicitly after reviewing contract changes"
    );
}
