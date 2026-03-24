use std::path::Path;

use crate::snapshot::FileComplexity;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    Rust,
    JsTs,
    Python,
    Go,
    Jvm, // Java, Kotlin, C#
    Generic,
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
        "java" | "kt" | "kts" | "cs" => Language::Jvm,
        _ => Language::Generic,
    }
}

pub fn analyse_file(path: &Path, content: &str) -> FileComplexity {
    let lang = detect_language(&path.to_string_lossy());
    analyse_content(content, lang)
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

    let cyclomatic_complexity = count_complexity(content);
    let public_methods = count_public_methods(content, lang);
    let properties = count_properties(content, lang);

    FileComplexity {
        total_lines,
        loc,
        cyclomatic_complexity,
        public_methods,
        properties,
    }
}

fn is_comment_line(trimmed: &str, lang: Language) -> bool {
    match lang {
        Language::Rust | Language::JsTs | Language::Go | Language::Jvm => {
            trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
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

fn count_public_methods(content: &str, lang: Language) -> u32 {
    let mut count = 0u32;
    match lang {
        Language::Rust => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("pub fn ") || t.starts_with("pub async fn ") {
                    count += 1;
                }
            }
        }
        Language::JsTs => {
            for line in content.lines() {
                let t = line.trim();
                if !t.starts_with("//")
                    && (t.starts_with("export function ")
                        || t.starts_with("export async function ")
                        || t.starts_with("export const ")
                        || (t.contains("public ") && t.contains('(')))
                {
                    count += 1;
                }
            }
        }
        Language::Python => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("def ") && !t.starts_with("def _") {
                    count += 1;
                }
            }
        }
        Language::Go => {
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
        }
        Language::Jvm => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("public ")
                    && t.contains('(')
                    && !t.starts_with("public class")
                    && !t.starts_with("public interface")
                    && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::Generic => {}
    }
    count
}

fn count_properties(content: &str, lang: Language) -> u32 {
    let mut count = 0u32;
    match lang {
        Language::Rust => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("pub ")
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
                {
                    count += 1;
                }
            }
        }
        Language::JsTs => {
            for line in content.lines() {
                let t = line.trim();
                if (t.starts_with("this.") && t.contains(" = "))
                    || ((t.starts_with("private ")
                        || t.starts_with("public ")
                        || t.starts_with("protected ")
                        || t.starts_with("readonly "))
                        && t.contains(':')
                        && !t.contains('('))
                {
                    count += 1;
                }
            }
        }
        Language::Python => {
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("self.") && t.contains(" = ") {
                    count += 1;
                }
            }
        }
        Language::Go => {
            for line in content.lines() {
                let t = line.trim();
                if t.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && (t.contains("string")
                        || t.contains("int")
                        || t.contains("bool")
                        || t.contains("float")
                        || t.contains("[]")
                        || t.contains("map["))
                    && !t.contains('(')
                    && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::Jvm => {
            for line in content.lines() {
                let t = line.trim();
                if (t.starts_with("private ")
                    || t.starts_with("public ")
                    || t.starts_with("protected "))
                    && !t.contains('(')
                    && t.contains(';')
                    && !t.starts_with("//")
                {
                    count += 1;
                }
            }
        }
        Language::Generic => {}
    }
    count
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
    fn detects_jvm() {
        assert!(matches!(detect_language("Foo.java"), Language::Jvm));
        assert!(matches!(detect_language("Bar.kt"), Language::Jvm));
        assert!(matches!(detect_language("Baz.cs"), Language::Jvm));
    }

    #[test]
    fn loc_skips_blank_and_comment_lines() {
        let content = "// comment\n\nfn main() {}\n    // indented comment\nlet x = 1;\n";
        let result = analyse_content(content, Language::Rust);
        assert_eq!(result.total_lines, 5);
        assert_eq!(result.loc, 2); // fn main(){} and let x = 1;
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
