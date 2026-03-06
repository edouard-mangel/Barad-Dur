# Barad-dur Backlog

## v2 — Planned

_Items actively being designed or scheduled for implementation._

(See `docs/plans/` for detailed designs once approved.)

## Future — Not Yet Scheduled

### Interactive Config Editor

**Priority**: Nice-to-have
**Depends on**: `.barad-dur.toml` config file (v2 infrastructure)

A guided CLI command (`barad-dur init` or `barad-dur config`) that helps users create or edit their `.barad-dur.toml` configuration file interactively. Should cover:

- Architectural grouping: define component mappings (regex → component name) with live preview of how current files would be grouped
- Team mapping: assign authors to teams, with auto-suggestions based on email domains
- Metric thresholds: customize score thresholds and weights
- Validation: warn on invalid regex, unmapped files, unknown authors

Could be a TUI (e.g. `ratatui`) or a simple question-and-answer flow (e.g. `dialoguer`).
