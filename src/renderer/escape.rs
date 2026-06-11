/// Escape serialized JSON for safe embedding inside a `<script>` element.
///
/// Browsers terminate a script block at the first `</` sequence regardless of
/// JS string context, so `</` must become `<\/` (a no-op inside JSON strings).
pub(crate) fn escape_json_for_script(json: &str) -> String {
    json.replace("</", "<\\/")
}

#[cfg(test)]
mod tests {
    use super::escape_json_for_script;

    #[test]
    fn escapes_closing_tag_sequences() {
        assert_eq!(
            escape_json_for_script(r#"{"a":"</script>"}"#),
            r#"{"a":"<\/script>"}"#
        );
    }

    #[test]
    fn escapes_every_occurrence() {
        assert_eq!(escape_json_for_script("</a></b>"), r"<\/a><\/b>");
    }

    #[test]
    fn leaves_safe_json_untouched() {
        let safe = r#"{"path":"src/a.rs","n":1}"#;
        assert_eq!(escape_json_for_script(safe), safe);
    }
}
