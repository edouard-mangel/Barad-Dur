use super::coupling::{COMMON_ADVICE, CONTENT_ADVICE, CONTROL_ADVICE, INHERITANCE_ADVICE};
use crate::scorer::types::{ReportTab, SortKey};

/// Where an action's link goes: a report tab, and optionally the column that
/// tab should sort by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ActionTarget {
    pub tab: ReportTab,
    pub sort_by: Option<SortKey>,
}

/// What the report tells a reader about one metric: the tab its action links
/// to and the advice it reads. Both live in one entry so a metric cannot be
/// given a link and forgotten for advice, or the other way round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Guidance {
    pub target: Option<ActionTarget>,
    pub advice: Option<&'static str>,
}

/// Shown for a metric whose entry carries no curated advice.
const GENERIC_ADVICE: &str = "Review and improve this metric";

impl Guidance {
    /// What an action uses for a name with no entry: no tab link, and
    /// `GENERIC_ADVICE`. No metric the analyzer emits gets this — a test
    /// requires every one to have curated advice — so it only covers a name
    /// the table has not been told about.
    pub(super) const UNGUIDED: Self = Self {
        target: None,
        advice: None,
    };

    const fn link(tab: ReportTab, sort_by: Option<SortKey>, advice: &'static str) -> Self {
        Self {
            target: Some(ActionTarget { tab, sort_by }),
            advice: Some(advice),
        }
    }

    const fn advice_only(advice: &'static str) -> Self {
        Self {
            target: None,
            advice: Some(advice),
        }
    }

    pub(super) fn advice_text(&self) -> &'static str {
        self.advice.unwrap_or(GENERIC_ADVICE)
    }

    pub(super) fn tab(&self) -> Option<ReportTab> {
        self.target.map(|target| target.tab)
    }

    pub(super) fn sort_by(&self) -> Option<SortKey> {
        self.target.and_then(|target| target.sort_by)
    }
}

/// The guidance for the metric a name identifies. `None` means no entry was
/// ever written for that name — a metric the table has not been told about.
/// Tests require every metric the analyzer emits to have an entry with
/// curated advice, so a new or renamed metric fails the build.
pub(super) fn guidance_for(metric_name: &str) -> Option<Guidance> {
    use ReportTab::{Age, Coupling, Hotspots, Ownership, Trends};
    use SortKey::{Authors, Complexity};

    Some(match metric_name {
        "Bus factor" => Guidance::link(
            Ownership,
            Some(Authors),
            "Increase code review coverage and pair programming to spread knowledge",
        ),
        "God objects" => Guidance::link(
            Hotspots,
            Some(Complexity),
            "Break down large files by extracting responsibilities into smaller modules",
        ),
        "Complex hotspots" => Guidance::link(
            Hotspots,
            Some(Complexity),
            "Prioritize refactoring files with both high complexity and high churn",
        ),
        "Long methods" => Guidance::link(
            Hotspots,
            Some(Complexity),
            "Extract smaller functions from the longest methods to improve readability",
        ),
        "Code biomarkers" => Guidance::link(
            Hotspots,
            Some(Complexity),
            "Reduce nesting depth by applying early returns and guard clauses",
        ),
        "Afferent coupling" => Guidance::link(
            Coupling,
            None,
            "Reduce dependents on high-Ca files by introducing abstractions or splitting modules",
        ),
        "Efferent coupling" => Guidance::link(
            Coupling,
            None,
            "Reduce imports by extracting shared interfaces or facades",
        ),
        "Circular dependencies" => Guidance::link(
            Coupling,
            None,
            "Break circular imports by extracting shared types into a separate module",
        ),
        "Change coupling smells" => Guidance::link(
            Coupling,
            None,
            "Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions",
        ),
        "Test safety net" => Guidance::link(
            Coupling,
            None,
            "Revive the paired tests of recently-changed source files — start with the lowest co-change pairs",
        ),
        "Knowledge distribution" => Guidance::link(
            Ownership,
            None,
            "Encourage cross-team contributions and rotate ownership",
        ),
        "Churn-ownership risk" => Guidance::link(
            Ownership,
            None,
            "Pair a second maintainer on the flagged high-churn single-owner files",
        ),
        "Ownership clarity" => Guidance::link(
            Ownership,
            None,
            "Assign clear code owners via CODEOWNERS file",
        ),
        "Collaboration patterns" => Guidance::link(
            Ownership,
            None,
            "Break directory silos through cross-functional reviews",
        ),
        "Cross-team coupling" => Guidance::link(
            Ownership,
            None,
            "Align ownership with change patterns — co-owning coupled files or splitting them along owner boundaries",
        ),
        "Knowledge loss" => Guidance::link(
            Ownership,
            None,
            "Schedule knowledge-transfer or documentation passes over the most unattributed files",
        ),
        "Code/test growth balance" => Guidance::link(
            Trends,
            None,
            "Pair recent source growth with tests — start with the listed untested second-half files",
        ),
        "Growth trend" => Guidance::link(
            Trends,
            None,
            "Monitor growth rate and plan for sustainable development",
        ),
        "Commit cadence" => Guidance::link(
            Trends,
            None,
            "Establish regular commit patterns and avoid large batches",
        ),
        "Code age" => Guidance::link(Age, None, "Plan modernization of oldest code sections"),
        "Refactoring ratio" => Guidance::link(
            Hotspots,
            None,
            "Balance new feature work with refactoring of existing code",
        ),
        "Contributor activity" => {
            Guidance::advice_only("Onboard more active contributors or check team health")
        }
        "Merge patterns" => {
            Guidance::advice_only("Review branching strategy for healthier merge patterns")
        }
        "Commit message quality" => {
            Guidance::advice_only("Adopt conventional commits or enforce message guidelines")
        }
        "History cleanliness" => Guidance::advice_only(
            "Clean up merge strategy and enforce linear history where possible",
        ),
        "Gitignore coverage" => Guidance::advice_only(
            "Review suspicious tracked files; ignore and untrack only confirmed credentials, local files, or generated artifacts",
        ),
        // The four Pressman kinds say the same thing the per-file coupling
        // actions say, from one wording.
        "Content coupling" => Guidance::link(Coupling, None, CONTENT_ADVICE),
        "Common coupling" => Guidance::link(Coupling, None, COMMON_ADVICE),
        "Control coupling" => Guidance::link(Coupling, None, CONTROL_ADVICE),
        "Inheritance coupling" => Guidance::link(Coupling, None, INHERITANCE_ADVICE),
        // The per-file decay annotation is drawn on the Hotspots tab.
        "Co-change reach trend" => Guidance::link(
            Hotspots,
            None,
            "Review the files whose co-change partners keep growing — their responsibilities are spreading; split them along those seams",
        ),
        "Firefighting ratio" => Guidance::advice_only(
            "Find what keeps forcing reverts and hotfixes — strengthen the tests or add a staging step before release",
        ),
        "Friction language ratio" => Guidance::advice_only(
            "Turn the hacks and workarounds named in commit messages into tracked debt items and schedule their removal",
        ),
        _ => return None,
    })
}
