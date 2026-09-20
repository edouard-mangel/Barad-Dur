use super::*;

fn classified(source: &str, lang: Language, expected: &[(&str, bool)]) {
    let grammar = match lang {
        Language::Java => tree_sitter_java::LANGUAGE.into(),
        Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Language::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        _ => unreachable!(),
    };
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar).unwrap();
    let tree = parser.parse(source, None).unwrap();
    assert!(
        !tree.root_node().has_error(),
        "{}",
        tree.root_node().to_sexp()
    );
    let evidence = ranges(tree.root_node(), source.as_bytes(), lang);
    let functions: Vec<_> = nodes(tree.root_node())
        .into_iter()
        .filter(|n| {
            matches!(
                n.kind(),
                "method_declaration" | "function_declaration" | "local_function_statement"
            )
        })
        .collect();
    assert!(
        !functions.is_empty(),
        "{source}: {}",
        tree.root_node().to_sexp()
    );
    for (name, wanted) in expected {
        let function = functions
            .iter()
            .find(|n| {
                n.child_by_field_name("name")
                    .is_some_and(|n| text(n, source.as_bytes()) == *name)
            })
            .unwrap_or_else(|| panic!("missing {name}"));
        let actual = evidence
            .iter()
            .any(|r| r.start <= function.start_byte() && r.end >= function.end_byte());
        assert_eq!(actual, *wanted, "{name}: {source}");
    }
}

#[test]
fn java_method_evidence_is_not_class_evidence() {
    classified(
        "import org.junit.Test; class C { @Test void yes() {} void no() {} }",
        Language::Java,
        &[("yes", true), ("no", false)],
    );
    classified(
        "class C { @org.junit.jupiter.api.Test void yes() {} void no() {} }",
        Language::Java,
        &[("yes", true), ("no", false)],
    );
}

#[test]
fn java_class_markers_and_direct_bases_include_helpers() {
    for prefix in [
        "@org.junit.jupiter.api.Nested class C",
        "@org.testng.annotations.Test class C",
        "class C extends junit.framework.TestCase",
    ] {
        classified(
            &format!("{prefix} {{ void helper() {{}} }} class P {{ void production() {{}} }}"),
            Language::Java,
            &[("helper", true), ("production", false)],
        );
    }
    classified("import junit.framework.TestCase; class C extends TestCase { void helper() {} } class D extends C { void production() {} }", Language::Java, &[("helper",true),("production",false)]);
}

#[test]
fn java_wildcards_lookalikes_conflicts_and_lexical_shadowing_are_not_evidence() {
    for source in [
        "import org.junit.*; class C { @Test void no() {} }",
        "class C { @Test void no() {} }",
        "import org.junit.Test; import other.Test; class C { @Test void no() {} }",
        "import org.junit.Test; class C { @interface Test {} @Test void no() {} }",
        "class C { String text = \"@org.junit.Test\"; /* @org.junit.Test */ void no() {} }",
        "@org.junit.Test class C { void no() {} }",
    ] {
        classified(source, Language::Java, &[("no", false)]);
    }
    classified("import org.junit.Test; class C { @interface Test {} @Test void no() {} } class D { @Test void yes() {} }", Language::Java, &[("no",false),("yes",true)]);
}

#[test]
fn csharp_namespace_alias_and_type_alias_resolve_methods_only() {
    for prefix in [
        "using Xunit;",
        "using F = Xunit.FactAttribute;",
        "using X = Xunit;",
    ] {
        let attr = if prefix.contains("F =") {
            "F"
        } else if prefix.contains("X =") {
            "X.Fact"
        } else {
            "FactAttribute"
        };
        classified(
            &format!("{prefix} class C {{ [{attr}] void yes() {{}} void no() {{}} }}"),
            Language::CSharp,
            &[("yes", true), ("no", false)],
        );
    }
    classified(
        "class C { [global::Xunit.Theory] void yes() {} void no() {} }",
        Language::CSharp,
        &[("yes", true), ("no", false)],
    );
}

#[test]
fn csharp_class_evidence_and_namespace_scoping() {
    classified("namespace A { using NUnit.Framework; [TestFixture] class C { void helper() {} } } namespace B { class C { [Test] void no() {} } }", Language::CSharp, &[("helper",true),("no",false)]);
    classified("using M = Microsoft.VisualStudio.TestTools.UnitTesting; [M.TestClassAttribute] class C { void helper() {} } class P { void no() {} }", Language::CSharp, &[("helper",true),("no",false)]);
}

#[test]
fn csharp_local_attributes_and_conflicting_aliases_are_not_evidence() {
    for source in [
        "using Xunit; class FactAttribute {} class C { [Fact] void no() {} }",
        "using F = Xunit.FactAttribute; using F = Other.FactAttribute; class C { [F] void no() {} }",
        "using NUnit.Framework; using Other; class C { [Other.Test] void no() {} }",
        "class C { [Fact] void no() {} }",
        "[Xunit.Fact] class C { void no() {} }",
        "class C { string s = \"[Xunit.Fact]\"; /* [Xunit.Fact] */ void no() {} }",
    ] { classified(source, Language::CSharp, &[("no",false)]); }
    classified("using Xunit; class C { class FactAttribute {} [Fact] void no() {} } class D { [Fact] void yes() {} }", Language::CSharp, &[("no",false),("yes",true)]);
}

#[test]
fn kotlin_alias_method_contains_nested_helper_but_not_separate_helper() {
    classified(
        "import kotlin.test.Test as Check\n@Check fun yes() { fun nested() {} }\nfun no() {}",
        Language::Kotlin,
        &[("yes", true), ("nested", true), ("no", false)],
    );
    classified(
        "import org.junit.jupiter.api.Test\nclass C {\n@Test fun yes() {}\nfun no() {}\n}",
        Language::Kotlin,
        &[("yes", true), ("no", false)],
    );
}

#[test]
fn kotlin_class_markers_and_direct_base() {
    for prefix in [
        "@org.junit.jupiter.api.Nested class C",
        "@org.testng.annotations.Test class C",
        "class C : junit.framework.TestCase()",
    ] {
        classified(
            &format!("{prefix} {{\nfun helper() {{}}\n}}\nfun no() {{}}"),
            Language::Kotlin,
            &[("helper", true), ("no", false)],
        );
    }
}

#[test]
fn kotlin_wildcards_and_local_annotation_names_do_not_resolve() {
    for source in [
        "import kotlin.test.*\n@Test fun no() {}",
        "import kotlin.test.Test\nannotation class Test {}\n@Test fun no() {}",
        "import kotlin.test.Test as Check\nimport other.Check\n@Check fun no() {}",
        "@kotlin.test.Test class C {\nfun no() {}\n}",
        "val s = \"@kotlin.test.Test\";\n/* @kotlin.test.Test */ fun no() {}",
    ] {
        classified(source, Language::Kotlin, &[("no", false)]);
    }
}

#[test]
fn lifecycle_and_parameterized_markers_are_attached_to_methods() {
    for marker in [
        "org.junit.BeforeClass",
        "org.junit.jupiter.api.TestTemplate",
        "org.junit.jupiter.params.ParameterizedTest",
        "org.testng.annotations.BeforeGroups",
    ] {
        classified(
            &format!("class C {{ @{marker} void yes() {{}} void no() {{}} }}"),
            Language::Java,
            &[("yes", true), ("no", false)],
        );
    }
    for marker in [
        "NUnit.Framework.TestCaseSourceAttribute",
        "NUnit.Framework.OneTimeTearDown",
        "Microsoft.VisualStudio.TestTools.UnitTesting.DataTestMethod",
        "Microsoft.VisualStudio.TestTools.UnitTesting.AssemblyInitializeAttribute",
        "Xunit.TheoryAttribute",
    ] {
        classified(
            &format!("class C {{ [{marker}] void yes() {{}} void no() {{}} }}"),
            Language::CSharp,
            &[("yes", true), ("no", false)],
        );
    }
    classified(
        "@kotlin.test.AfterTest fun yes() {}\nfun no() {}",
        Language::Kotlin,
        &[("yes", true), ("no", false)],
    );
}

#[test]
fn managed_evidence_reaches_existing_function_metrics_for_all_extensions() {
    use crate::metrics::complexity::analyse_file;
    use std::path::Path;
    for (extension, source) in [
        (
            "java",
            "class C { @org.junit.Test void yes() { if (true) {} } void no() {} }",
        ),
        (
            "cs",
            "class C { [Xunit.Fact] void yes() { if (true) {} } void no() {} }",
        ),
        (
            "kt",
            "@kotlin.test.Test fun yes() { if (true) {} }\nfun no() {}",
        ),
        (
            "kts",
            "@kotlin.test.Test fun yes() { if (true) {} }\nfun no() {}",
        ),
    ] {
        let metrics = analyse_file(Path::new(&format!("src/production.{extension}")), source);
        assert_eq!(metrics.functions.len(), 2, "{extension}");
        let yes = metrics.functions.iter().find(|f| f.name == "yes").unwrap();
        let no = metrics.functions.iter().find(|f| f.name == "no").unwrap();
        assert!(yes.is_test, "{extension}");
        assert!(!no.is_test, "{extension}");
        assert!(
            yes.cyclomatic_complexity > no.cyclomatic_complexity,
            "measurements retained for {extension}"
        );
    }
}

#[test]
fn malformed_annotations_do_not_create_evidence() {
    for (grammar, language, source) in [
        (
            tree_sitter_java::LANGUAGE.into(),
            Language::Java,
            "class C { @org.junit.Test( void no() {} }",
        ),
        (
            tree_sitter_c_sharp::LANGUAGE.into(),
            Language::CSharp,
            "class C { [Xunit.Fact( void no() {} }",
        ),
        (
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            Language::Kotlin,
            "@kotlin.test.Test( fun no() {}",
        ),
    ] {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&grammar).unwrap();
        let tree = parser.parse(source, None).unwrap();
        assert!(tree.root_node().has_error());
        assert!(ranges(tree.root_node(), source.as_bytes(), language).is_empty());
    }
}

#[test]
fn generic_parameter_shadows_are_limited_to_their_declaration() {
    classified(
        "import org.junit.Test; class C<Test> { @Test void no() {} } class D { @Test void yes() {} }",
        Language::Java,
        &[("no", false), ("yes", true)],
    );
    classified(
        "using Xunit; class C<Fact> { [Fact] void no() {} } class D { [Fact] void yes() {} }",
        Language::CSharp,
        &[("no", false), ("yes", true)],
    );
    classified(
        "import kotlin.test.Test\nclass C<Test> {\n@Test fun no() {}\n}\n@Test fun yes() {}",
        Language::Kotlin,
        &[("no", false), ("yes", true)],
    );
    classified(
        "import org.junit.Test; class C { <Test> void generic() {} @Test void yes() {} }",
        Language::Java,
        &[("yes", true)],
    );
}

#[test]
fn kotlin_test_markers_do_not_establish_java_framework_evidence() {
    for marker in ["Test", "BeforeTest", "AfterTest"] {
        classified(
            &format!("class C {{ @kotlin.test.{marker} void no() {{}} }}"),
            Language::Java,
            &[("no", false)],
        );
    }
}

#[test]
fn targeted_attributes_and_annotations_do_not_mark_method_bodies() {
    classified(
        "class C { [return: Xunit.Fact] void no() {} [Xunit.Fact] void yes() {} }",
        Language::CSharp,
        &[("no", false), ("yes", true)],
    );
    classified(
        "@get:kotlin.test.Test fun no() {}\n@kotlin.test.Test fun yes() {}",
        Language::Kotlin,
        &[("no", false), ("yes", true)],
    );
}

#[test]
fn errors_in_method_bodies_and_enclosing_parse_nodes_reject_markers() {
    for (grammar, language, source) in [
        (
            tree_sitter_java::LANGUAGE.into(),
            Language::Java,
            "class C { @org.junit.Test void no() { broken ???; } }",
        ),
        (
            tree_sitter_java::LANGUAGE.into(),
            Language::Java,
            "class C { @org.junit.Test void no() {} ???",
        ),
        (
            tree_sitter_c_sharp::LANGUAGE.into(),
            Language::CSharp,
            "class C { [Xunit.Fact] void no() {}",
        ),
        (
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            Language::Kotlin,
            "class C { @kotlin.test.Test fun no() {}",
        ),
    ] {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&grammar).unwrap();
        let tree = parser.parse(source, None).unwrap();
        assert!(tree.root_node().has_error());
        assert!(
            ranges(tree.root_node(), source.as_bytes(), language).is_empty(),
            "{source}"
        );
    }
}
