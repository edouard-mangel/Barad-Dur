# Design: CI/CD Self-Contained HTML Report

**Date:** 2026-03-07
**Status:** Approved

## Problem

Barad-dûr currently produces either a CLI text output or JSON. Neither is suitable as a human-readable CI/CD artifact. Teams running `barad-dur` in GitHub Actions, GitLab CI, or similar want a single HTML file they can upload as an artifact and open in a browser without any server or dependencies.

## Solution

Add a `--html` flag to `barad-dur analyze` that produces a self-contained HTML file. The file embeds the report data as a JSON blob and renders it with vanilla JS + inline SVG. No React build step, no CDN dependencies, no external assets.

## CLI

```bash
# Write to file
barad-dur analyze . --html -o report.html

# Pipe to file
barad-dur analyze . --html > report.html
```

## CI/CD Example (GitHub Actions)

```yaml
- name: Analyze repository
  run: barad-dur analyze . --html -o barad-dur-report.html

- name: Upload report artifact
  uses: actions/upload-artifact@v4
  with:
    name: barad-dur-report
    path: barad-dur-report.html
    retention-days: 30
```

## Architecture

### New files
- `src/renderer/html.rs` — HTML template renderer

### Modified files
- `src/cli.rs` — add `--html: bool` to `AnalyzeArgs`
- `src/renderer/mod.rs` — add `pub mod html`
- `src/main.rs` — wire `renderer::html::render()` when `args.html` is true

### Data flow
```
AnalysisReport → serde_json::to_string() → injected as window.REPORT
             → HTML template string (in html.rs)
             → single String returned → written to file or stdout
```

## HTML Structure

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>{repo_name} — Barad-dûr Report</title>
  <style>/* all CSS inline */</style>
</head>
<body>
  <script>const REPORT = {json};</script>
  <div id="app"></div>
  <script>/* vanilla JS renderer */</script>
</body>
</html>
```

## Visual Design

Mirrors the React dashboard:
- Background: `#080a0f`
- Accent: `#f59e0b` (amber)
- Text: `#e2e8f0`, muted `rgba(148,163,184,0.5)`
- Font: system monospace + system sans-serif (no CDN fonts for offline compat)
- Tabs: Overview | Hotspots | Coupling | Ownership | Age

## Sections

### Overview Tab
- Score gauge (SVG arc)
- Radar chart (SVG polygon, vanilla JS)
- Category cards with metric rows (expandable)
- Top actions panel

### Hotspots Tab
- Scatter SVG plot (complexity vs churn, bubble = LOC)
- Sortable table (score, churn, CC, LOC, methods, props)

### Coupling Tab
- Table with inline progress bars

### Ownership Tab
- Stacked author bar per file

### Age Tab
- Files sorted by staleness, colored age bands

## Score Colors
- ≥ 71: `#10b981` (green)
- 41–70: `#f59e0b` (amber)
- ≤ 40: `#ef4444` (red)

## Trade-offs

| Approach | Pros | Cons |
|----------|------|------|
| Vanilla JS template (chosen) | Tiny binary, offline, no build dep | More Rust template code |
| Embedded React build | Same visual fidelity | 500KB+ HTML, build dep |
| External CDN React | Small binary | Requires internet |

## Testing

- Unit test: `render()` returns non-empty string with key markers
- Smoke test: open generated HTML in browser, verify all tabs render
- CI smoke: add `test-html-report` job to verify the flag works end-to-end
