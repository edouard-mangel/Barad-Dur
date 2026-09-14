# Roadmap tasks by ascending difficulty

Date: 2026-09-11

Based on the [project roadmap](../../README.md#roadmap), the [backlog](../BACKLOG.md), and a quick code inspection. Difficulty includes implementation, validation, and unresolved design choices. These are preliminary estimates, not implementation commitments or completion statuses.

| Rank | Task | Difficulty | Quick analysis |
|---|---|---|---|
| 1 | **Rust toolchain and dependency refresh** | Low–medium | Mostly version updates and compatibility fixes, followed by existing checks. Tree-sitter upgrades could increase the work because grammar changes can affect analysis results. |
| 2 | **Dashboard file exclusions** | Low–medium | File lists and exclusion rules already exist. Add selection controls and generate matching `.baraddurignore` entries. This assumes scores update after rerunning analysis; live recalculation would be substantially harder. |
| 3 | **macOS support** | Medium | Some platform handling already exists, including browser opening. Main work is native build/test coverage, dependency compatibility, and release packaging. Access to a macOS runner is the main uncertainty. |
| 4 | **Per-commit score gate in CI** | Medium | `gate` already resolves baseline refs and compares coupling findings. Extend this to scores, ensuring both commits use comparable collection, configuration, and time inputs. Two analyses also need a runtime check. |
| 5 | **Interactive configuration editor** | Medium–high | Existing configuration parsing and validation help. However, the backlog includes component previews and author/team mapping, which extend beyond simply editing existing settings. Preserving existing configuration adds work. |
| 6 | **Analysis data in a dedicated repository** | Medium–high | Storage currently derives paths from the analyzed repository. Requires separating configuration, disposable caches, and persistent history, plus repository identity, migration, and handling concurrent history updates. |
| 7 | **GitHub/GitLab API integration for PR data** | High | Existing GitHub integration fetches only repository metadata. PR collection needs provider adapters, authentication, pagination, caching, failure handling, and a shared representation of reviews and approvals. |
| 8 | **Multi-repo dashboard** | High | The dashboard currently stores and displays one report. Requires multiple-report storage, repository navigation, comparable analysis windows, and explicit aggregation rules. Dedicated storage would help; manual multi-file upload would reduce scope. |
| 9 | **PR/MR analysis** | High | Depends on PR data integration. The difficult part is defining trustworthy turnaround and approval metrics across drafts, reopened requests, repeated reviews, and incomplete data, then exposing the evidence clearly. |
| 10 | **End-of-life map** | Very high | Requires version extraction across ecosystems, product/release matching, external support-policy data, caching, and honest handling of unknown status. Broad dependency coverage is much harder than a limited runtime/framework list. |

The most scope-sensitive positions are **macOS**, **multi-repo dashboard**, and **end-of-life mapping**. This is an effort ranking; implementation order should also account for dependencies, particularly **API integration → PR/MR analysis**.
