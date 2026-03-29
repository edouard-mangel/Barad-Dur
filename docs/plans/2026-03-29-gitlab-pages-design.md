# GitLab Pages Landing Page — Design Document

**Date:** 2026-03-29
**Status:** Approved

---

## 1. Goal

Replace the current GitLab Pages index (which serves the raw HTML analysis report) with a marketing landing page that explains the tool. The report remains accessible via a direct link.

---

## 2. Approach

Static HTML committed to the repository (`site/index.html`). No build step. The CI `pages` job copies it to `public/index.html` and places the self-analysis report at `public/report.html`.

Rationale: consistent with the existing `--html` report delivery model (self-contained, no external dependencies), zero CI complexity, easy to maintain.

---

## 3. CI Changes

`pages` job in `.gitlab-ci.yml`:

```yaml
script:
  - mkdir -p public
  - cp site/index.html public/index.html
  - cp barad-dur-report.html public/report.html
  - cp barad-dur-report.json public/report.json
```

---

## 4. Page Structure

| Section | Content |
|---|---|
| Header | Eye of Sauron SVG logo + `barad-dur` wordmark + "Live report →" nav link |
| Hero | Tagline, 2-sentence pitch, `cargo install` code block, "View live report →" CTA |
| What it analyzes | 4-card grid: Health / Team / Evolution / Hygiene |
| How it works | 3-step flow: run → metrics → output formats |
| Output formats | CLI, JSON, HTML — one paragraph each |
| Footer | GitLab link |

---

## 5. Visual Design

Mirrors [lacrafterie.tech](https://lacrafterie.tech):

- **Font:** Atkinson Hyperlegible (Google Fonts)
- **Accent:** `#2337ff`
- **Body text:** `rgb(15, 18, 25)`
- **Muted text:** `rgb(96, 115, 159)`
- **Max-width:** 720px centered
- **Base font size:** 20px, line-height 1.7
- **Hero background:** linear gradient light gray → white (600px)

Logo: SVG Eye of Sauron — stylized elliptical eye with slit pupil and radiating lines, monochrome, ~80px, used in the header.
