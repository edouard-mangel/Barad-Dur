//! Deeply nested source must not overflow a rayon worker's default stack.
use std::path::Path;

/// rayon's default worker stack; collection runs every file on such a thread.
const WORKER_STACK: usize = 2 * 1024 * 1024;

fn nested(open: &str, close: &str, depth: usize) -> String {
    format!("{}1{}", open.repeat(depth), close.repeat(depth))
}

fn extracts_on_a_worker_stack(path: &'static str, source: String, function: &'static str) {
    let names = std::thread::Builder::new()
        .stack_size(WORKER_STACK)
        .spawn(move || {
            crate::metrics::complexity::analyse_file(Path::new(path), &source)
                .functions
                .into_iter()
                .map(|f| f.name)
                .collect::<Vec<_>>()
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(names, [function], "{path}");
}

#[test]
fn deeply_nested_javascript_does_not_overflow_the_worker_stack() {
    let body = nested("[", "]", 5_000);
    extracts_on_a_worker_stack(
        "src/deep.js",
        format!("function f() {{ return {body}; }}\n"),
        "f",
    );
}

#[test]
fn deeply_nested_javascript_assignment_target_does_not_overflow_the_worker_stack() {
    let target = nested("[", "]", 5_000).replace('1', "x");
    extracts_on_a_worker_stack(
        "src/deep_target.js",
        format!("function f(y) {{ let x; {target} = y; }}\n"),
        "f",
    );
}

#[test]
fn deeply_nested_python_assignment_target_does_not_overflow_the_worker_stack() {
    let target = nested("[", "]", 10_000).replace('1', "x");
    extracts_on_a_worker_stack(
        "src/deep_target.py",
        format!("def f(y):\n    {target} = y\n"),
        "f",
    );
}

#[test]
fn deeply_nested_javascript_call_chain_does_not_overflow_the_worker_stack() {
    let chain = ".b()".repeat(5_000);
    extracts_on_a_worker_stack(
        "src/deep_chain.js",
        format!("function f(x) {{ return x{chain}; }}\n"),
        "f",
    );
}

#[test]
fn deeply_nested_php_does_not_overflow_the_worker_stack() {
    let body = nested("[", "]", 5_000);
    extracts_on_a_worker_stack(
        "src/deep.php",
        format!("<?php function f() {{ return {body}; }}\n"),
        "f",
    );
}

#[test]
fn deeply_nested_python_does_not_overflow_the_worker_stack() {
    let body = nested("[", "]", 10_000);
    extracts_on_a_worker_stack("src/deep.py", format!("def f():\n    return {body}\n"), "f");
}

#[test]
fn deeply_nested_rust_does_not_overflow_the_worker_stack() {
    let body = nested("(", ")", 10_000);
    extracts_on_a_worker_stack("src/deep.rs", format!("fn f() -> i32 {{ {body} }}\n"), "f");
}

#[test]
fn deeply_nested_java_does_not_overflow_the_worker_stack() {
    let body = nested("(", ")", 5_000);
    extracts_on_a_worker_stack(
        "src/Deep.java",
        format!("class Deep {{ int f() {{ return {body}; }} }}\n"),
        "f",
    );
}

#[test]
fn deeply_nested_csharp_does_not_overflow_the_worker_stack() {
    let body = nested("(", ")", 5_000);
    extracts_on_a_worker_stack(
        "src/Deep.cs",
        format!("class Deep {{ int F() {{ return {body}; }} }}\n"),
        "F",
    );
}

#[test]
fn deeply_nested_kotlin_does_not_overflow_the_worker_stack() {
    let body = nested("(", ")", 5_000);
    extracts_on_a_worker_stack(
        "src/Deep.kt",
        format!("fun f(): Int {{ return {body} }}\n"),
        "f",
    );
}
