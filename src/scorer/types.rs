use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::metrics::callgraph::CallGraphReport;
use crate::metrics::churn::ChurnTimelineReport;
use crate::metrics::coupling::CouplingFindingCounts;
use crate::metrics::CategoryResult;
use crate::scoring::ScoreThresholds;

/// Direction of a per-entity metric series (complexity, coupling degree,
/// churn) across backfill samples — distinct from `VelocityDirection`,
/// which classifies the aggregate report score on an absolute scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum EntityTrendDirection {
    Growing,
    Shrinking,
    Stable,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
#[non_exhaustive]
pub struct HotspotFile {
    pub path: String,
    /// What kind of file this is (source / test / config / docs / other) —
    /// lets renderers separate code hotspots from CI-file and test churn.
    pub role: crate::metrics::file_role::FileRole,
    pub churn_count: usize,
    pub bug_commit_count: usize,
    pub loc: usize,
    pub total_lines: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    pub hotspot_score: f64,
    /// Ch. 8 decay annotation (trends M3): present when this file's
    /// distinct co-change partner count at least doubled half-over-half
    /// and cleared `decay_min_partners`. Structured so renderers own the
    /// wording (e.g. "3 → 9").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub coupling_trend: Option<CouplingTrend>,
    /// Pressman coupling findings in this file, per kind. Content includes
    /// barrel-bypass findings when `content_barrel_rule` is on — the same
    /// gating as `CouplingEvidence::finding_counts`, so the two views never
    /// disagree.
    pub content_findings: usize,
    pub common_findings: usize,
    pub control_findings: usize,
    pub inheritance_findings: usize,
    /// Commits touching the file per 1/12 of the analysis window (oldest first).
    pub churn_timeline: Vec<u32>,
    /// Complexity trend across backfill history (Ch. 6), if any exists for
    /// this file. `None` when no `entity_trends.json` history exists yet,
    /// or this file has never been in the top-N hotspots of a backfill
    /// sample.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity_trend: Option<EntityTrendDirection>,
    /// Churn-count trend across backfill history (Ch. 14) — independent of
    /// `complexity_trend`; a file can grow in complexity while its churn
    /// stays flat, or vice versa. Same `None` contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub churn_trend: Option<EntityTrendDirection>,
}

/// Half-over-half distinct co-change partner counts for a file flagged as
/// growing reach (Ch. 8 decay, trends M3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct CouplingTrend {
    pub first_half_partners: usize,
    pub second_half_partners: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
#[non_exhaustive]
pub struct CouplingPair {
    pub file_a: String,
    pub file_b: String,
    pub co_changes: usize,
    pub coupling_pct: f64,
    pub cross_boundary: bool,
    pub is_test_pair: bool,
    /// Net in-window lines (added − deleted) per side, non-merge commits
    /// only — Ch. 14's "which coupled member actually grew" (trends M1).
    pub growth_a: i64,
    pub growth_b: i64,
    /// Coupling-degree trend across backfill history (Ch. 8), if any
    /// exists for this pair. `None` when no history exists yet, or this
    /// pair has never qualified as a change-coupling smell in a backfill
    /// sample.
    ///
    /// Distinct from `HotspotFile::coupling_trend`, which is the within-run
    /// half-over-half reach decay of a single file: this one is a
    /// cross-backfill-sample direction for a *pair*'s co-change degree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupling_trend: Option<EntityTrendDirection>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct AuthorShare {
    pub name: String,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct FileOwnership {
    pub path: String,
    pub authors: Vec<AuthorShare>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct FileAge {
    pub path: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub days_since_modified: i64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct AuthorCard {
    pub name: String,
    pub email: String,
    pub commit_count: usize,
    pub files_owned: usize,
    pub lines_owned: usize,
    pub avg_commit_quality: f64,
    pub top_files: Vec<String>,
    pub last_active: chrono::DateTime<chrono::Utc>,
    pub days_since_active: i64,
    pub directories_touched: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct CrisisFile {
    pub path: String,
    pub crisis_commit_count: usize,
    pub total_commit_count: usize,
    pub crisis_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct DirConcentration {
    pub dir: String,
    pub file_count: usize,
    pub loc: usize,
    pub pct_of_total: f64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct DeadFile {
    pub path: String,
    pub days_since_modified: i64,
    pub churn_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct VelocityBucket {
    pub week_start: String,
    pub commit_count: usize,
    pub author_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct AuditReport {
    pub crisis_files: Vec<CrisisFile>,
    pub dir_concentration: Vec<DirConcentration>,
    pub dead_files: Vec<DeadFile>,
    pub velocity_buckets: Vec<VelocityBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct FileCouplingMetrics {
    pub path: String,
    pub ca: usize,
    pub ce: usize,
    pub instability: f64,
}

/// One directed edge of the static import graph: `from` imports `to`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct ImportEdge {
    pub from: String,
    pub to: String,
}

/// Metadata about the remote repository origin (populated when a URL is given).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct RemoteMeta {
    pub url: String,
    pub stars: Option<u64>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub open_issues: Option<u64>,
}

/// The report tab an action links to. Serialized as the lowercase word the
/// HTML report capitalises and looks up among its tab names, so a variant
/// here must name a tab that exists there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ReportTab {
    Hotspots,
    Coupling,
    Ownership,
    Trends,
    Age,
}

/// The column an action asks the target tab to sort by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum SortKey {
    Authors,
    Complexity,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct ActionItem {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub target_tab: Option<ReportTab>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub sort_by: Option<SortKey>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
#[non_exhaustive]
pub struct AnalysisReport {
    pub repo_name: String,
    pub branch: String,
    pub time_window_months: u32,
    pub total_commits: usize,
    pub total_authors: usize,
    pub total_files: usize,
    /// `None` when no category is measurable (see `CategoryResult::score`).
    pub overall_score: Option<u32>,
    pub categories: Vec<CategoryResult>,
    pub top_actions: Vec<ActionItem>,
    /// Per-file coupling refactoring suggestions (Pressman M6), ranked by
    /// severity rung then corroboration. Surfaced in the Coupling tab.
    pub coupling_actions: Vec<ActionItem>,
    pub remote_meta: Option<RemoteMeta>,
    pub file_hotspots: Vec<HotspotFile>,
    pub coupling_pairs: Vec<CouplingPair>,
    pub author_ownership: Vec<FileOwnership>,
    pub file_ages: Vec<FileAge>,
    pub author_cards: Vec<AuthorCard>,
    pub history: Vec<HistoryEntry>,
    pub dep_ecosystem_reports: Vec<crate::deps::EcosystemReport>,
    pub audit: Option<AuditReport>,
    pub per_file_coupling: Vec<FileCouplingMetrics>,
    pub import_edges: Vec<ImportEdge>,
    /// Import cycles as sorted member-file lists (depth 1 and 2).
    pub import_cycles: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub coupling_finding_counts: Option<CouplingFindingCounts>,
    /// Call-graph summary (design D7); `None` = no call data collected.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub call_graph: Option<CallGraphReport>,
    /// Day-bucketed churn shape (trends M1); `None` = no data.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub churn_timeline: Option<ChurnTimelineReport>,
    pub score_thresholds: ScoreThresholds,
    /// Effective Long Methods thresholds, serialized so the report guidance
    /// renders the rule that was actually applied instead of restating the
    /// defaults — same contract as `score_thresholds`.
    pub long_method_thresholds: LongMethodThresholds,
}

#[cfg(test)]
impl AnalysisReport {
    /// A report with nothing in it: every list empty, every option `None`,
    /// every count zero, default thresholds. A fixture names the fields it
    /// tests and takes the rest from here (`..AnalysisReport::blank()`), so a
    /// new report field is added in one place instead of in every renderer's
    /// tests. `report_contract.rs` deliberately stays explicit: the wire
    /// contract should make a new field a conscious decision.
    pub(crate) fn blank() -> Self {
        Self {
            repo_name: String::new(),
            branch: String::new(),
            time_window_months: 0,
            total_commits: 0,
            total_authors: 0,
            total_files: 0,
            overall_score: None,
            categories: Vec::new(),
            top_actions: Vec::new(),
            coupling_actions: Vec::new(),
            remote_meta: None,
            file_hotspots: Vec::new(),
            coupling_pairs: Vec::new(),
            author_ownership: Vec::new(),
            file_ages: Vec::new(),
            author_cards: Vec::new(),
            history: Vec::new(),
            dep_ecosystem_reports: Vec::new(),
            audit: None,
            per_file_coupling: Vec::new(),
            import_edges: Vec::new(),
            import_cycles: Vec::new(),
            coupling_finding_counts: None,
            call_graph: None,
            churn_timeline: None,
            score_thresholds: Default::default(),
            long_method_thresholds: Default::default(),
        }
    }
}

/// The four thresholds the Long Methods predicate reads, as applied to this run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
#[non_exhaustive]
pub struct LongMethodThresholds {
    pub cc: u32,
    pub cc_floor: u32,
    pub loc: usize,
    pub ui_loc: usize,
}

impl From<&crate::config::HealthThresholds> for LongMethodThresholds {
    fn from(health: &crate::config::HealthThresholds) -> Self {
        Self {
            cc: health.long_method_cc,
            cc_floor: health.long_method_cc_floor,
            loc: health.long_method_loc,
            ui_loc: health.long_method_ui_loc,
        }
    }
}

// Defaults delegate to the config defaults so the report can never state a
// rule the analyser would not have applied.
impl Default for LongMethodThresholds {
    fn default() -> Self {
        Self::from(&crate::config::HealthThresholds::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct HistoryCounts {
    pub commits: usize,
    pub files: usize,
    pub authors: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub content_coupling: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub common_coupling: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub control_coupling: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub inheritance_coupling: Option<usize>,
}

/// Version of the scoring formula that produced a `HistoryEntry`.
///
/// History is *derived*: `backfill` recomputes every entry through the
/// current metrics and scorer, so entries are a cache of what today's
/// formula says about past commits, not a record of what was observed.
/// That makes an entry from an older formula a stale computation rather
/// than history worth keeping — mixing the two would report a formula
/// change as if the code had moved.
///
/// Bump this whenever a scoring change alters the numbers. `load_history`
/// then archives the old file and starts fresh, and `barad-dur backfill`
/// regenerates the series. There is deliberately never more than one
/// version in play, so nothing downstream reasons about a boundary.
///
/// 2: prevalence and count bands blended across the transition range
///    (was: count bands capping prevalence at every population size).
/// 3: gitignore coverage uses component-aware path rules (source, docs,
///    binary, and template files no longer count as credentials by name;
///    OS metadata and wrapped certificate files now do).
/// 4: a category with no scored metric is unscored (`null`) and excluded
///    from the overall, whose weights renormalise over the measurable
///    categories (was: such a category counted as 100 at full weight).
/// 5: Long Methods uses complexity-gated LOC thresholds, with a higher LOC
///    threshold for declarative .tsx/.jsx UI code.
/// 6: Kotlin imports include qualified paths and wildcard package members,
///    changing the structural evidence used by coupling and health scores.
pub const HISTORY_SCHEMA_VERSION: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct HistoryEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "head", alias = "commit")]
    pub head: String,
    /// `None` when no category could be scored (nothing measurable).
    pub overall_score: Option<u32>,
    /// `None` values are categories that were present but unscored.
    #[serde(rename = "category_scores", alias = "categories")]
    pub categories: HashMap<String, Option<u32>>,
    #[serde(default)]
    pub metrics: HashMap<String, u32>,
    #[serde(default)]
    pub counts: HistoryCounts,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-types", ts(optional))]
    pub source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_report_has_no_content_to_leak_into_fixtures() {
        // Fixtures inherit everything they do not name; any list, text, or
        // count that is not empty here would sit silently in every one of them.
        use serde_json::Value;
        let json = serde_json::to_value(AnalysisReport::blank()).unwrap();
        let with_content: Vec<&String> = json
            .as_object()
            .unwrap()
            .iter()
            .filter(|(_, value)| match value {
                Value::Array(items) => !items.is_empty(),
                Value::String(text) => !text.is_empty(),
                Value::Number(n) => n.as_f64() != Some(0.0),
                Value::Bool(flag) => *flag,
                Value::Null | Value::Object(_) => false,
            })
            .map(|(key, _)| key)
            .collect();
        assert!(
            with_content.is_empty(),
            "blank report has: {with_content:?}"
        );
    }

    #[test]
    fn report_tabs_serialize_as_the_lowercase_words_the_html_report_switches_on() {
        // The HTML report capitalises the first letter and looks the word up
        // among its tab names; the JSON wording is a contract, not a detail.
        let words: Vec<serde_json::Value> = [
            ReportTab::Hotspots,
            ReportTab::Coupling,
            ReportTab::Ownership,
            ReportTab::Trends,
            ReportTab::Age,
        ]
        .iter()
        .map(|tab| serde_json::to_value(tab).unwrap())
        .collect();
        assert_eq!(
            words,
            ["hotspots", "coupling", "ownership", "trends", "age"]
        );
    }

    #[test]
    fn sort_keys_serialize_as_lowercase_words() {
        let words: Vec<serde_json::Value> = [SortKey::Authors, SortKey::Complexity]
            .iter()
            .map(|key| serde_json::to_value(key).unwrap())
            .collect();
        assert_eq!(words, ["authors", "complexity"]);
    }

    #[test]
    fn an_action_without_a_target_omits_both_keys() {
        let action = ActionItem {
            text: "t".into(),
            target_tab: None,
            sort_by: None,
        };
        let json = serde_json::to_value(&action).unwrap();
        assert!(json.get("target_tab").is_none() && json.get("sort_by").is_none());
    }

    #[test]
    fn an_action_with_a_target_serializes_the_words() {
        let action = ActionItem {
            text: "t".into(),
            target_tab: Some(ReportTab::Ownership),
            sort_by: Some(SortKey::Authors),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["target_tab"], "ownership");
        assert_eq!(json["sort_by"], "authors");
    }

    #[test]
    fn history_counts_omit_absent_coupling_fields() {
        let counts = HistoryCounts {
            commits: 1,
            files: 2,
            authors: 3,
            content_coupling: None,
            common_coupling: None,
            control_coupling: None,
            inheritance_coupling: None,
        };
        let json = serde_json::to_value(&counts).unwrap();
        assert!(
            json.get("content_coupling").is_none(),
            "None must serialize as absent, not null"
        );
        assert!(json.get("common_coupling").is_none());
        assert!(json.get("control_coupling").is_none());
        assert!(json.get("inheritance_coupling").is_none());
    }

    #[test]
    fn history_counts_serialize_present_coupling_fields() {
        let counts = HistoryCounts {
            commits: 1,
            files: 2,
            authors: 3,
            content_coupling: Some(0),
            common_coupling: Some(4),
            control_coupling: Some(7),
            inheritance_coupling: Some(2),
        };
        let json = serde_json::to_value(&counts).unwrap();
        assert_eq!(json["content_coupling"], 0);
        assert_eq!(json["common_coupling"], 4);
        assert_eq!(json["control_coupling"], 7);
        assert_eq!(json["inheritance_coupling"], 2);
    }
}
