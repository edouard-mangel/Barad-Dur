# Evolution: HTML Report Readability — Light/Dark Mode Toggle

**Date**: 2026-06-05
**Feature ID**: html-report-readability
**Branch**: feat/dashboard-e2e-pause-polling (incidental — no dedicated branch required)

## Summary

The self-contained `--html` report was dark-mode only with hardcoded hex colors. Users viewing the
report in bright environments had no way to switch to a lighter palette. This feature adds a
persistent light/dark mode toggle to the HTML report, controlled by a header button and stored in
`localStorage`.

Because the report is a self-contained single-file artifact with no external dependencies, the
entire theme system — CSS variables, JS toggle logic, and the button — is inlined into the
generated HTML at render time.

## Business Context

Improved ergonomics for users who review barad-dur reports in daylight conditions or who prefer
light color schemes. The toggle is non-destructive: dark mode remains the default, preserving the
existing experience for users who have not expressed a preference.

## Implementation Steps

### 01-01: Extract dark-mode colors to CSS variables

All hardcoded hex UI colors in `src/renderer/html/css.rs` were replaced with CSS custom properties
declared in the `body` rule (`--bg-primary`, `--bg-secondary`, `--bg-card`, `--border-color`,
`--text-primary`, `--text-muted`, and related tokens — 15 variables total). Hardcoded hex values
were removed from layout and UI color rules; only semantic tokens that have no theme variant (e.g.,
data-visualization colors already managed by the CBF system) were left untouched.

### 01-02: Add `body.light` selector overriding all theme variables

A `body.light { ... }` block was added to `css.rs` that overrides every CSS variable from 01-01
with light-mode equivalents (`--bg-primary: #f8fafc`, `--text-primary: #1e293b`,
`--border-color: #e2e8f0`, etc.). The block is self-contained and isolated — dark-mode defaults
remain on `body`, and `body.light` layers overrides via CSS specificity with no `!important` use.

### 01-03: Compose CBF overrides with light context

A `body.light.cbf { ... }` block was added to re-apply CBF semantic token overrides (sky-blue
`#38bdf8` and related accessible colors) in the combined CBF + light-mode context. Without this
block, the generic `body.light` palette would have silently overridden CBF-specific accessible
colors, producing inaccessible contrast ratios for colorblind users in light mode. Composition is
via CSS specificity (`body.light.cbf` beats `body.light`) — no duplication, no `!important`.

### 02-01: Add `initTheme()` and `toggleTheme()` functions

`js_shared.rs` was extended with two functions inlined into the generated HTML:

- `initTheme()`: reads `localStorage.theme` first; falls back to `prefers-color-scheme`; falls back
  to dark default. Called once before any DOM rendering to prevent flash of wrong theme.
- `toggleTheme()`: toggles `body.light` class and writes `'light'` to `localStorage` or removes
  the key for dark, ensuring persistence across page reloads.

### 02-02: Add theme toggle button to header

`renderApp()` in the header chips area now creates a theme toggle button (id `theme-btn`,
`aria-label="Toggle theme"`) using the `el()` helper exclusively — no `innerHTML`, consistent with
the project's security hook. The button displays ☀ in dark mode and ☾ in light mode, and updates
both the icon and label on click by calling `toggleTheme()`. It is positioned next to the existing
CBF accessibility button.

An L4 refactoring opportunity was taken during this step: the duplicated `buildMethodologyDetails()`
copy-paste in the Details tab was extracted into a single reusable function, eliminating ~30 lines
of duplication with no behavior change.

## Key Decisions

**CSS variable naming convention**: variables follow the `--bg-*` / `--text-*` / `--border-*`
namespace pattern already present in the CBF system, keeping the token vocabulary consistent.

**`localStorage` key**: `'theme'` (simple string, not namespaced). Acceptable for a single-page
self-contained report with no competing localStorage consumers.

**Fallback precedence** (`localStorage` > `prefers-color-scheme` > dark default): `localStorage`
wins because explicit user intent beats system preference. `prefers-color-scheme` is respected as
the second-best signal. Dark default is the safe fallback that preserves prior behavior for users
who have never interacted with the toggle.

**`body.light.cbf` composition via CSS specificity**: adding a combined selector with higher
specificity than either `body.light` or `body.cbf` alone is the canonical CSS approach for
composing two modifier classes. It avoids `!important`, avoids duplication of the base CBF rules,
and makes the composition intent explicit.

**`el()` helper enforced (no `innerHTML`)**: all DOM construction in the JS layer uses the `el()`
factory. The project's security audit hook flags `innerHTML` usage. Consistent use of `el()` also
makes the code auditable — no risk of accidental XSS through report data values.

## Test Coverage

- **Total tests**: 724 (6 new, all in `src/renderer/html/tests.rs` and `tests_extra.rs`)
- New tests: `html_has_light_mode_css_block`, `html_cbf_light_compose`,
  `html_theme_init_function_present`, `html_theme_toggle_button_present`, and 2 supporting
  assertions for CSS token presence.
- All 724 tests pass with `RUSTFLAGS=-D warnings`.

## Mutation Testing

**Result**: SKIP (justified). `cargo-mutants` found 0 mutants to test. All production changes are
Rust string literal constants (`pub const CSS: &str = r#"..."#` and similar). `cargo-mutants`
mutates Rust expressions, conditions, and function bodies — it does not mutate string literal
contents, so no mutants were generated.

String-presence assertions are the appropriate quality gate for HTML renderer code: the rendered
HTML string _is_ the contract. The 6 new tests perform substantive assertions that would catch real
regressions in the theme system.

## Lessons Learned

**`cargo-mutants` cannot mutate Rust string constants.** For modules whose entire production
surface is string constants (CSS/JS embedded in Rust), mutation testing will always produce zero
mutants. This is not a coverage gap — it is a tool boundary. The correct response is to write
string-presence tests (assert that key substrings appear in the rendered output) and document the
skip as justified, rather than chasing a kill-rate metric that cannot be satisfied by definition.

This pattern applies to all renderer modules in this project: `css.rs`, `js_shared.rs`,
`js_authors.rs`, `js_audit.rs`. Any future feature touching only these files should expect the same
mutation skip outcome.
