//! Collection cost must grow roughly linearly with file size. Each test times the
//! same generated shape at `n` and `4 * n`: linear work scales about 4x (measured
//! up to 4.4x with parsing overhead), a quadratic pass 16x and a cubic one 64x.
//! The ratio uses the fastest of three runs per size, so a noisy neighbour cannot
//! fake either outcome.
use std::path::Path;
use std::time::{Duration, Instant};

const REPEATS: usize = 3;
const MAX_RATIO: f64 = 8.0;

fn fastest(path: &str, source: &str) -> Duration {
    (0..REPEATS)
        .map(|_| {
            let started = Instant::now();
            std::hint::black_box(super::analyse_file(Path::new(path), source));
            started.elapsed()
        })
        .min()
        .unwrap()
}

fn assert_scales_linearly(path: &str, n: usize, generate: impl Fn(usize) -> String) {
    let small = fastest(path, &generate(n));
    let large = fastest(path, &generate(4 * n));
    let ratio = large.as_secs_f64() / small.as_secs_f64();
    assert!(
        ratio < MAX_RATIO,
        "{path}: n={n} took {small:?}, 4n took {large:?} (ratio {ratio:.1})"
    );
}

/// The shape of generated parsers (e.g. ANTLR's serialized ATN): one long
/// left-nested `+` chain of string literals.
fn concatenation(n: usize) -> String {
    vec!["\"ab\""; n].join(" + ")
}

#[test]
fn java_string_concatenation_scales_linearly() {
    assert_scales_linearly("src/Parser.java", 100, |n| {
        format!(
            "class Parser {{ static final String ATN = {}; }}\n",
            concatenation(n)
        )
    });
}

#[test]
fn csharp_string_concatenation_scales_linearly() {
    assert_scales_linearly("src/Parser.cs", 100, |n| {
        format!(
            "class Parser {{ const string Atn = {}; }}\n",
            concatenation(n)
        )
    });
}

#[test]
fn kotlin_string_concatenation_scales_linearly() {
    assert_scales_linearly("src/Parser.kt", 100, |n| {
        format!("object Parser {{ val atn = {} }}\n", concatenation(n))
    });
}

#[test]
fn javascript_call_chain_scales_linearly() {
    assert_scales_linearly("src/query.js", 100, |n| {
        format!("function build(x) {{ return x{}; }}\n", ".b()".repeat(n))
    });
}

#[test]
fn go_methods_on_one_type_scale_linearly() {
    assert_scales_linearly("src/api.go", 100, |n| {
        let methods: String = (0..n)
            .map(|i| format!("func (c *Client) Op{i}() int {{ return c.f }}\n"))
            .collect();
        format!("package api\ntype Client struct {{ f int }}\n{methods}")
    });
}

#[test]
fn python_module_expression_scales_linearly() {
    assert_scales_linearly("src/table.py", 800, |n| {
        format!("x = {}\n", vec!["1"; n].join(" + "))
    });
}
