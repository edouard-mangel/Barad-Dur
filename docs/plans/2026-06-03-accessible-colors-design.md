# Accessible Colors (CBF Toggle) — Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a GUI toggle in the HTML report that switches between the default palette and a color-blind-friendly (CBF) palette, persisted in `localStorage`.

**Architecture:** CSS custom properties define 4 semantic color tokens in `:root`. A `body.cbf` class overrides the two green tokens with sky-blue. All JS files reference `var(--c-*)` in inline styles instead of hardcoded hex. A toggle button in the page header flips the class and saves the preference.

**Tech Stack:** Pure CSS custom properties + vanilla JS (no new dependencies). All changes in `src/renderer/html/`.

---

## Why CSS custom properties, not class swaps

JS in this report sets colors via inline styles (`node.style.color = '#10b981'`). CSS classes cannot override inline styles, but CSS custom properties used *inside* inline styles (`node.style.color = 'var(--c-good)'`) are resolved at paint time and are overridable by a body-class rule. This is the only approach that works without rewriting the DOM on toggle.

## CBF Palette

| Token | Default | CBF (body.cbf) | Rationale |
|---|---|---|---|
| `--c-good` | `#10b981` emerald | `#38bdf8` sky-blue | Green→Blue eliminates red/green confusion |
| `--c-good-lo` | `#22c55e` green | `#38bdf8` sky-blue | Same semantic, same fix |
| `--c-warn` | `#f59e0b` amber | `#f59e0b` unchanged | Amber is already distinguishable |
| `--c-danger` | `#ef4444` red | `#ef4444` unchanged | Red vs blue is safe for all CVD types |

Only the green tokens change. Amber and red need no adjustment because the failure mode is red/green confusion (deuteranopia/protanopia), not red/amber or amber/blue.

## Files to touch

| File | Change |
|---|---|
| `css.rs` | Add `:root { --c-good: …; … }` and `body.cbf { --c-good: …; … }` |
| `js_shared.rs` | `scoreColor()`, `defaultScoreHints`, `buildCatCard()`, `scoreBar()` |
| `js_age.rs` | Legend color array + `fileAgeColor()` return values |
| `js_coupling.rs` | Legend color array + bar/span inline colors |
| `js_authors.rs` | Legend array + `authorActivityColor()` + `scoreColor()` |
| `js_hotspots.rs` | Legend color array |
| `js_ownership.rs` | Legend color array |
| `js_trends.rs` | `scoreColor()` + backfill/live dot colors |
| `js_overview.rs` | Add toggle button to header + `localStorage` init |

## Toggle button

Placed in the `.meta-chips` row of the page header (built in `js_overview.rs`).

- **Off state:** label `◑ CBF`, muted style
- **On state:** label `◐ Default`, active border

On click:
```js
document.body.classList.toggle('cbf');
localStorage.setItem('cbf-palette', document.body.classList.contains('cbf') ? '1' : '');
```

On page load (top of `js_overview.rs` init):
```js
if (localStorage.getItem('cbf-palette')) document.body.classList.add('cbf');
```

## Testing

Existing renderer tests assert specific hex strings — update them to assert `var(--c-good)` etc. Add one new test that verifies:
1. The `:root` block with all 4 tokens is present in the CSS output.
2. The `body.cbf` override block is present.
3. No bare `#10b981` or `#22c55e` remain in the JS output.
