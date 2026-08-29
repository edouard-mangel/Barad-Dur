use std::path::Path;

use crate::snapshot::FileComplexity;

/// `#[non_exhaustive]` for the reason `CouplingPair` and `AnalysisReport`
/// already carry it (0.19.0): this enum gains a variant every time a
/// language is taught to the collector, and on an exhaustive public enum
/// each of those is a breaking change. Ruby and C/C++ are already queued.
///
/// New variants are appended, never inserted: the discriminants are public
/// too, and moving one breaks downstream `as isize` casts.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Language {
    Rust,
    JsTs,
    Python,
    Go,
    Java,
    Kotlin,
    CSharp,
    Generic,
    Php,
}

pub fn detect_language(path: &str) -> Language {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "rs" => Language::Rust,
        "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs" => Language::JsTs,
        "py" => Language::Python,
        "go" => Language::Go,
        "java" => Language::Java,
        "kt" | "kts" => Language::Kotlin,
        "cs" => Language::CSharp,
        "php" => Language::Php,
        _ => Language::Generic,
    }
}

pub fn analyse_content(content: &str, lang: Language) -> FileComplexity {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let loc = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !is_comment_line(t, lang)
        })
        .count();

    // Keyword counting is meaningless in prose/config (a LICENSE or YAML
    // file matches `if`/`case`/`||` by accident and ranks as a complex
    // hotspot) — an unknown language gets line counts only.
    let cyclomatic_complexity = match lang {
        Language::Generic => 0,
        _ => count_complexity(content),
    };
    let public_methods = count_public_methods(content, lang);
    let properties = count_properties(content, lang);

    FileComplexity {
        total_lines,
        loc,
        cyclomatic_complexity,
        public_methods,
        properties,
        ..Default::default()
    }
}

fn is_comment_line(trimmed: &str, lang: Language) -> bool {
    match lang {
        Language::Rust
        | Language::JsTs
        | Language::Go
        | Language::Java
        | Language::Kotlin
        | Language::CSharp => {
            trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
        }
        // PHP takes the C-family forms, plus `#` — but only when it is not
        // `#[`, which opens a PHP 8 attribute. Attributes are code, and
        // counting them as comments quietly undercounts LOC.
        Language::Php => {
            trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || (trimmed.starts_with('#') && !trimmed.starts_with("#["))
        }
        Language::Python => trimmed.starts_with('#'),
        Language::Generic => trimmed.starts_with("//") || trimmed.starts_with('#'),
    }
}

fn count_complexity(content: &str) -> u32 {
    let keywords = [
        " if ", "\tif ", "(if ", "else if", "elif ", " for ", "\tfor ", " while ", "\twhile ",
        " match ", " switch ", " case ", " catch ", " except ", " loop ", "&&", "||", " ?? ",
    ];
    let mut count = 0u32;
    for kw in &keywords {
        count += content.matches(kw).count() as u32;
    }
    count
}

fn pub_methods_rust(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.starts_with("pub fn ") || t.starts_with("pub async fn ")
        })
        .count() as u32
}

fn pub_methods_jsts(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            !t.starts_with("//")
                && (t.starts_with("export function ")
                    || t.starts_with("export async function ")
                    || t.starts_with("export const ")
                    || (t.contains("public ") && t.contains('(')))
        })
        .count() as u32
}

fn pub_methods_python(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.starts_with("def ") && !t.starts_with("def _")
        })
        .count() as u32
}

fn pub_methods_go(content: &str) -> u32 {
    let mut count = 0u32;
    for line in content.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("func ") {
            let name_start = if rest.starts_with('(') {
                rest.find(')').map(|i| rest[i + 1..].trim()).unwrap_or("")
            } else {
                rest
            };
            if name_start
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                count += 1;
            }
        }
    }
    count
}

fn pub_methods_jvm(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.starts_with("public ")
                && t.contains('(')
                && !t.starts_with("public class")
                && !t.starts_with("public interface")
                && !t.starts_with("//")
        })
        .count() as u32
}

fn count_public_methods(content: &str, lang: Language) -> u32 {
    match lang {
        Language::Rust => pub_methods_rust(content),
        Language::JsTs => pub_methods_jsts(content),
        Language::Python => pub_methods_python(content),
        Language::Go => pub_methods_go(content),
        Language::Java | Language::Kotlin | Language::CSharp | Language::Php => {
            pub_methods_jvm(content)
        }
        Language::Generic => 0,
    }
}

fn props_rust(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.starts_with("pub ")
                && t.contains(':')
                && !t.starts_with("pub fn")
                && !t.starts_with("pub async fn")
                && !t.starts_with("pub struct")
                && !t.starts_with("pub enum")
                && !t.starts_with("pub mod")
                && !t.starts_with("pub use")
                && !t.starts_with("pub type")
                && !t.starts_with("pub trait")
                && !t.starts_with("pub const")
                && !t.starts_with("pub static")
                && !t.starts_with("//")
        })
        .count() as u32
}

fn props_jsts(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            (t.starts_with("this.") && t.contains(" = "))
                || ((t.starts_with("private ")
                    || t.starts_with("public ")
                    || t.starts_with("protected ")
                    || t.starts_with("readonly "))
                    && t.contains(':')
                    && !t.contains('('))
        })
        .count() as u32
}

fn props_python(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.starts_with("self.") && t.contains(" = ")
        })
        .count() as u32
}

fn props_go(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            t.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && (t.contains("string")
                    || t.contains("int")
                    || t.contains("bool")
                    || t.contains("float")
                    || t.contains("[]")
                    || t.contains("map["))
                && !t.contains('(')
                && !t.starts_with("//")
        })
        .count() as u32
}

fn props_jvm(content: &str) -> u32 {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            (t.starts_with("private ") || t.starts_with("public ") || t.starts_with("protected "))
                && !t.contains('(')
                && t.contains(';')
                && !t.starts_with("//")
        })
        .count() as u32
}

fn count_properties(content: &str, lang: Language) -> u32 {
    match lang {
        Language::Rust => props_rust(content),
        Language::JsTs => props_jsts(content),
        Language::Python => props_python(content),
        Language::Go => props_go(content),
        Language::Java | Language::Kotlin | Language::CSharp | Language::Php => props_jvm(content),
        Language::Generic => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_language() {
        assert!(matches!(detect_language("src/main.rs"), Language::Rust));
        assert!(matches!(detect_language("lib.rs"), Language::Rust));
    }

    #[test]
    fn detects_js_ts() {
        assert!(matches!(detect_language("app.ts"), Language::JsTs));
        assert!(matches!(detect_language("index.jsx"), Language::JsTs));
    }

    #[test]
    fn detects_python() {
        assert!(matches!(detect_language("script.py"), Language::Python));
    }

    #[test]
    fn detects_go() {
        assert!(matches!(detect_language("main.go"), Language::Go));
    }

    #[test]
    fn detects_java() {
        assert!(matches!(detect_language("Foo.java"), Language::Java));
    }

    #[test]
    fn php_attributes_are_code_not_comments() {
        // `#` is a PHP comment, but PHP 8 attributes are `#[...]` — code
        // that happens to start with the legacy comment character. Treating
        // them as comments silently undercounts LOC, which feeds hotspot
        // size scoring. The validation repo has 70 attributes and zero `#`
        // comments, so getting this backwards costs real accuracy.
        assert!(!is_comment_line("#[DataProvider('cases')]", Language::Php));
        assert!(!is_comment_line("#[OA\\Post(path: '/x')]", Language::Php));
        assert!(is_comment_line("# legacy hash comment", Language::Php));
        assert!(is_comment_line("// slash comment", Language::Php));
        assert!(is_comment_line("/* block */", Language::Php));
        assert!(is_comment_line("* continuation", Language::Php));
    }

    #[test]
    fn detects_php() {
        assert_eq!(detect_language("app/Models/User.php"), Language::Php);
    }

    #[test]
    fn detects_kotlin() {
        assert!(matches!(detect_language("Bar.kt"), Language::Kotlin));
        assert!(matches!(detect_language("App.kts"), Language::Kotlin));
    }

    #[test]
    fn detects_csharp() {
        assert!(matches!(detect_language("Baz.cs"), Language::CSharp));
    }

    #[test]
    fn loc_skips_blank_and_comment_lines() {
        let content = "// comment\n\nfn main() {}\n    // indented comment\nlet x = 1;\n";
        let result = analyse_content(content, Language::Rust);
        assert_eq!(result.total_lines, 5);
        assert_eq!(result.loc, 2); // fn main(){} and let x = 1;
    }

    #[test]
    fn generic_content_has_zero_complexity() {
        // Prose and config files hit every keyword pattern by accident —
        // a LICENSE ranked as a complex hotspot. Unknown language ⇒ line
        // counts only.
        let content = "You may copy, modify || distribute this work if and\n\
                       only if you match each case of the conditions while\n\
                       the license terms hold && remain in effect.\n";
        let result = analyse_content(content, Language::Generic);
        assert_eq!(result.cyclomatic_complexity, 0);
        assert_eq!(result.total_lines, 3);
        assert!(result.loc > 0);
    }

    #[test]
    fn cyclomatic_complexity_counts_decision_points() {
        let content = " if x { } else if y { } for i in v { } while z { } match a { _ => {} }";
        let result = analyse_content(content, Language::Rust);
        // if, else if, for, while, match = at least 5
        assert!(result.cyclomatic_complexity >= 5);
    }

    #[test]
    fn public_methods_rust() {
        let content = "pub fn foo() {}\nfn bar() {}\npub fn baz() {}\n";
        let result = analyse_content(content, Language::Rust);
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn public_methods_typescript() {
        let content = "export function foo() {}\nfunction bar() {}\nexport const baz = () => {}\n";
        let result = analyse_content(content, Language::JsTs);
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn properties_rust_struct_fields() {
        let content = "struct Foo {\n    pub x: i32,\n    pub y: String,\n    z: bool,\n}\n";
        let result = analyse_content(content, Language::Rust);
        assert_eq!(result.properties, 2);
    }
}
