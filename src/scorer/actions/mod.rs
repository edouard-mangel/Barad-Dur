//! Suggestions layered onto a finished report, one responsibility per module:
//! `top` ranks the weakest metrics, `guidance` says what each reads and links
//! to, `coupling` advises per file on Pressman findings, and `refactoring`
//! proposes splits for god objects.

mod coupling;
mod guidance;
mod refactoring;
mod top;

pub(in crate::scorer) use coupling::generate_coupling_actions;
pub(in crate::scorer) use refactoring::generate_refactoring_actions;
pub(in crate::scorer) use top::generate_top_actions;

#[cfg(test)]
use {
    crate::scorer::types::{ReportTab, SortKey},
    guidance::{guidance_for, ActionTarget, Guidance},
    refactoring::{group_methods_by_prefix, matches_group_prefix, MethodGroup, GROUPING_PREFIXES},
    std::path::Path,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every metric name the analyzer emits for the categories `gate` scores,
    /// from an empty snapshot: the names do not depend on the data.
    fn emitted_metric_names() -> Vec<String> {
        use crate::analysis::{calculate, AnalysisInputs, CategorySelection};
        use crate::config::{CategoryWeights, Thresholds};

        let snapshot = crate::metrics::testutil::make_snapshot();
        let thresholds = Thresholds::default();
        let weights = CategoryWeights::default().as_weight_pairs();
        let reach = crate::metrics::coupling::growing_coupling_reach(
            &snapshot,
            thresholds.coupling.decay_min_partners,
        );
        let god_objects = crate::metrics::health::god_object_files(&snapshot, &thresholds.health);
        calculate(&AnalysisInputs {
            reference_time: chrono::Utc::now(),
            snapshot: &snapshot,
            selection: CategorySelection::GATE,
            thresholds: &thresholds,
            weights: &weights,
            dependency_evidence: &[],
            god_objects: &god_objects,
            coupling_reach: &reach,
        })
        .categories
        .into_iter()
        .flat_map(|category| category.metrics.into_iter().map(|metric| metric.name))
        .collect()
    }

    fn routed(tab: ReportTab, sort_by: Option<SortKey>) -> Option<ActionTarget> {
        Some(ActionTarget { tab, sort_by })
    }

    #[test]
    fn every_emitted_metric_has_a_guidance_decision() {
        // A metric the guidance table has never heard of used to fall through
        // `_ =>` arms into generic advice and no tab link, with no signal.
        let missing: Vec<String> = emitted_metric_names()
            .into_iter()
            .filter(|name| guidance_for(name).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "metrics with no guidance decision: {missing:?}"
        );
    }

    #[test]
    fn a_name_no_metric_emits_has_no_guidance() {
        assert!(guidance_for("not a metric").is_none());
    }

    #[test]
    fn routes_are_pinned_for_every_metric_that_links_to_a_tab() {
        use ReportTab::{Age, Coupling, Hotspots, Ownership, Trends};
        use SortKey::{Authors, Complexity};
        let pinned = [
            ("Bus factor", routed(Ownership, Some(Authors))),
            ("God objects", routed(Hotspots, Some(Complexity))),
            ("Complex hotspots", routed(Hotspots, Some(Complexity))),
            ("Long methods", routed(Hotspots, Some(Complexity))),
            ("Code biomarkers", routed(Hotspots, Some(Complexity))),
            ("Afferent coupling", routed(Coupling, None)),
            ("Efferent coupling", routed(Coupling, None)),
            ("Circular dependencies", routed(Coupling, None)),
            ("Change coupling smells", routed(Coupling, None)),
            ("Test safety net", routed(Coupling, None)),
            ("Knowledge distribution", routed(Ownership, None)),
            ("Churn-ownership risk", routed(Ownership, None)),
            ("Ownership clarity", routed(Ownership, None)),
            ("Collaboration patterns", routed(Ownership, None)),
            ("Cross-team coupling", routed(Ownership, None)),
            ("Knowledge loss", routed(Ownership, None)),
            ("Code/test growth balance", routed(Trends, None)),
            ("Growth trend", routed(Trends, None)),
            ("Commit cadence", routed(Trends, None)),
            ("Code age", routed(Age, None)),
            ("Refactoring ratio", routed(Hotspots, None)),
        ];
        for (name, expected) in pinned {
            let guidance = guidance_for(name).unwrap_or_else(|| panic!("no guidance for {name}"));
            assert_eq!(guidance.target, expected, "route for {name}");
        }
    }

    /// The advice an action for `name` reads, falling back the way the
    /// production call sites do.
    fn advice_of(name: &str) -> &'static str {
        guidance_for(name)
            .unwrap_or(Guidance::UNGUIDED)
            .advice_text()
    }

    #[test]
    fn curated_advice_is_pinned_for_representative_metrics() {
        // One entry per behaviour class: a linked metric, an advisory metric
        // that carries a list of findings, and a metric with advice only.
        assert_eq!(
            advice_of("Code/test growth balance"),
            "Pair recent source growth with tests — start with the listed untested second-half files"
        );
        assert_eq!(
            advice_of("Churn-ownership risk"),
            "Pair a second maintainer on the flagged high-churn single-owner files"
        );
        assert_eq!(
            advice_of("Cross-team coupling"),
            "Align ownership with change patterns — co-owning coupled files or splitting them along owner boundaries"
        );
        assert_eq!(
            advice_of("Bus factor"),
            "Increase code review coverage and pair programming to spread knowledge"
        );
        assert_eq!(
            advice_of("Knowledge loss"),
            "Schedule knowledge-transfer or documentation passes over the most unattributed files"
        );
        assert_eq!(
            advice_of("Test safety net"),
            "Revive the paired tests of recently-changed source files — start with the lowest co-change pairs"
        );
    }

    #[test]
    fn a_name_nobody_wrote_guidance_for_reads_the_generic_line() {
        assert_eq!(advice_of("not a metric"), "Review and improve this metric");
    }

    #[test]
    fn every_emitted_metric_has_curated_advice() {
        // The generic line is a fallback for a name the table has never heard
        // of, not an answer for a metric the analyzer really emits: a scored
        // one can rank among the weakest and would read it in the top actions.
        let generic: Vec<String> = emitted_metric_names()
            .into_iter()
            .filter(|name| guidance_for(name).and_then(|g| g.advice).is_none())
            .collect();
        assert!(
            generic.is_empty(),
            "metrics that would read the generic line: {generic:?}"
        );
    }

    #[test]
    fn pressman_coupling_metrics_share_the_per_file_advice() {
        // One wording per concept: the metric-level action and the per-file
        // coupling action say the same thing about the same finding kind.
        use super::coupling::{COMMON_ADVICE, CONTENT_ADVICE, CONTROL_ADVICE, INHERITANCE_ADVICE};
        assert_eq!(advice_of("Content coupling"), CONTENT_ADVICE);
        assert_eq!(advice_of("Common coupling"), COMMON_ADVICE);
        assert_eq!(advice_of("Control coupling"), CONTROL_ADVICE);
        assert_eq!(advice_of("Inheritance coupling"), INHERITANCE_ADVICE);
    }

    #[test]
    fn the_new_advice_reads_as_an_instruction_about_its_own_metric() {
        assert_eq!(
            advice_of("Content coupling"),
            "Reaches into another module's internals — import through the module's public interface instead."
        );
        assert_eq!(
            advice_of("Firefighting ratio"),
            "Find what keeps forcing reverts and hotfixes — strengthen the tests or add a staging step before release"
        );
        assert_eq!(
            advice_of("Friction language ratio"),
            "Turn the hacks and workarounds named in commit messages into tracked debt items and schedule their removal"
        );
        assert_eq!(
            advice_of("Co-change reach trend"),
            "Review the files whose co-change partners keep growing — their responsibilities are spreading; split them along those seams"
        );
    }

    #[test]
    fn the_newly_guided_metrics_link_where_a_reader_can_act_on_them() {
        use ReportTab::{Coupling, Hotspots};
        for name in [
            "Content coupling",
            "Common coupling",
            "Control coupling",
            "Inheritance coupling",
        ] {
            assert_eq!(
                guidance_for(name).unwrap().target,
                routed(Coupling, None),
                "{name}"
            );
        }
        // The decay annotation is drawn on the Hotspots tab, per file.
        assert_eq!(
            guidance_for("Co-change reach trend").unwrap().target,
            routed(Hotspots, None)
        );
        // Hygiene metrics have no tab of their own, like the rest of the category.
        for name in ["Firefighting ratio", "Friction language ratio"] {
            assert_eq!(guidance_for(name).unwrap().target, None, "{name}");
        }
    }

    #[test]
    fn gitignore_action_requires_review_and_confirmation() {
        let action = advice_of("Gitignore coverage");
        assert_eq!(
            action,
            "Review suspicious tracked files; ignore and untrack only confirmed credentials, local files, or generated artifacts"
        );
        assert!(!action.contains("remove from tracking"));
    }

    use crate::metrics::{CategoryResult, MetricValue, RawValue};

    #[test]
    fn top_actions_carry_the_tab_and_sort_key_of_their_metric() {
        // The sort key is what makes a drill-through open the Authors or
        // Hotspots tab already ordered; the table alone does not prove the
        // action carries it.
        let scored = |name: &str, score: u32| MetricValue {
            name: name.to_string(),
            description: "low".to_string(),
            raw_value: RawValue::Integer(1),
            score: Some(score),
        };
        let categories = vec![CategoryResult {
            name: "Health".to_string(),
            score: Some(20),
            metrics: vec![
                scored("Bus factor", 10),
                scored("God objects", 20),
                scored("Knowledge distribution", 30),
            ],
        }];

        let carried: Vec<(Option<ReportTab>, Option<SortKey>)> = generate_top_actions(&categories)
            .iter()
            .map(|action| (action.target_tab, action.sort_by))
            .collect();

        assert_eq!(
            carried,
            [
                (Some(ReportTab::Ownership), Some(SortKey::Authors)),
                (Some(ReportTab::Hotspots), Some(SortKey::Complexity)),
                (Some(ReportTab::Ownership), None),
            ]
        );
    }

    #[test]
    fn a_metric_scoring_exactly_eighty_is_not_weak_enough_for_an_action() {
        let category = |score: u32| CategoryResult {
            name: "Health".to_string(),
            score: Some(score),
            metrics: vec![MetricValue {
                name: "Bus factor".to_string(),
                description: "d".to_string(),
                raw_value: RawValue::Integer(1),
                score: Some(score),
            }],
        };
        assert_eq!(generate_top_actions(&[category(80)]).len(), 0);
        assert_eq!(generate_top_actions(&[category(79)]).len(), 1);
    }

    #[test]
    fn top_actions_picks_worst() {
        let categories = vec![
            CategoryResult {
                name: "Health".to_string(),
                score: Some(50),
                metrics: vec![
                    MetricValue {
                        name: "Bus factor".to_string(),
                        description: "bad".to_string(),
                        raw_value: RawValue::Integer(1),
                        score: Some(20),
                    },
                    MetricValue {
                        name: "Churn hotspots".to_string(),
                        description: "ok".to_string(),
                        raw_value: RawValue::Count(0),
                        score: Some(90),
                    },
                ],
            },
            CategoryResult {
                name: "Team".to_string(),
                score: Some(40),
                metrics: vec![MetricValue {
                    name: "Knowledge distribution".to_string(),
                    description: "bad".to_string(),
                    raw_value: RawValue::Float(0.8),
                    score: Some(15),
                }],
            },
        ];

        let actions = generate_top_actions(&categories);
        assert!(!actions.is_empty());
        assert!(actions[0].text.contains("Knowledge distribution"));
    }

    #[test]
    fn top_actions_includes_advisory_metrics_with_findings() {
        // Metrics the recalibration made advisory (score: None) still carry
        // real findings; their curated advice must stay reachable.
        let categories = vec![CategoryResult {
            name: "Team".to_string(),
            score: Some(100),
            metrics: vec![
                MetricValue {
                    name: "Collaboration patterns".to_string(),
                    description: "8/9 top-level directories...".to_string(),
                    raw_value: RawValue::Count(8),
                    score: None,
                },
                MetricValue {
                    name: "Cross-team coupling".to_string(),
                    description: "1 cross-team pair".to_string(),
                    raw_value: RawValue::List(vec!["a.rs <-> b.rs".to_string()]),
                    score: None,
                },
            ],
        }];
        let actions = generate_top_actions(&categories);
        let silo = actions
            .iter()
            .find(|a| a.text.contains("Break directory silos"))
            .expect("collaboration-patterns advice must be reachable");
        assert!(silo.text.contains("advisory"), "{}", silo.text);
        assert_eq!(silo.target_tab, Some(ReportTab::Ownership));
        assert!(actions
            .iter()
            .any(|a| a.text.contains("Align ownership with change patterns")));
    }

    #[test]
    fn top_actions_skips_advisory_metrics_without_findings() {
        // N/A metrics, zero counts, and empty evidence lists are not
        // findings — no advisory action.
        let metric = |name: &str, raw_value| MetricValue {
            name: name.to_string(),
            description: "d".to_string(),
            raw_value,
            score: None,
        };
        let categories = vec![CategoryResult {
            name: "Team".to_string(),
            score: Some(100),
            metrics: vec![
                metric("Bus factor", RawValue::Text("N/A".to_string())),
                metric("Collaboration patterns", RawValue::Count(0)),
                metric("Cross-team coupling", RawValue::List(vec![])),
                metric("Growth trend", RawValue::Integer(42)),
            ],
        }];
        assert!(generate_top_actions(&categories).is_empty());
    }

    use crate::config::CouplingThresholds;
    use crate::snapshot::{CouplingFinding, CouplingKind, RepoSnapshot};
    use std::path::PathBuf;

    fn snap_with(findings: Vec<CouplingFinding>) -> RepoSnapshot {
        let mut s = crate::metrics::testutil::make_snapshot();
        s.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
        s.file_metrics.insert(
            PathBuf::from("src/a.rs"),
            crate::snapshot::FileComplexity::default(),
        );
        s.coupling_findings = findings;
        s
    }
    fn finding(path: &str, kind: CouplingKind) -> CouplingFinding {
        CouplingFinding {
            path: PathBuf::from(path),
            line: Some(1),
            kind,
            evidence: "e".into(),
        }
    }

    #[test]
    fn coupling_actions_empty_when_no_findings() {
        let s = snap_with(vec![]);
        assert!(
            generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                &s,
                &CouplingThresholds::default()
            ))
            .is_empty()
        );
    }

    #[test]
    fn coupling_actions_order_content_common_control() {
        let s = snap_with(vec![
            finding("src/ctrl.rs", CouplingKind::Control),
            finding("src/glob.rs", CouplingKind::Common),
            finding("src/int.rs", CouplingKind::Content),
        ]);
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
        let files: Vec<&str> = acts.iter().map(|a| a.text.as_str()).collect();
        assert!(files[0].contains("src/int.rs") && files[0].contains("worst: content"));
        assert!(files[1].contains("src/glob.rs") && files[1].contains("worst: common"));
        assert!(files[2].contains("src/ctrl.rs") && files[2].contains("worst: control"));
        assert_eq!(acts[0].target_tab, Some(ReportTab::Coupling));
        assert!(acts[0].sort_by.is_none());
    }

    #[test]
    fn coupling_actions_worst_rung_wins_for_mixed_file() {
        // Two mixed files with findings inserted in OPPOSITE severity orders.
        // Both must resolve to their most-severe kind (common) regardless of
        // insertion order — `src/mix2.rs` (Common then Control) is the case a
        // dropped `.min()` / last-write-wins would get wrong.
        let s = snap_with(vec![
            finding("src/mix1.rs", CouplingKind::Control),
            finding("src/mix1.rs", CouplingKind::Common),
            finding("src/mix2.rs", CouplingKind::Common),
            finding("src/mix2.rs", CouplingKind::Control),
        ]);
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
        assert_eq!(acts.len(), 2);
        for a in &acts {
            assert!(a.text.contains("worst: common"), "{}", a.text);
            assert!(a.text.contains("2 finding(s)"), "{}", a.text);
        }
    }

    #[test]
    fn coupling_actions_corroborated_first_within_rung() {
        // Two Common files, one corroborated (co-changes cross-boundary).
        let mut s = snap_with(vec![
            finding("src/dormant.rs", CouplingKind::Common),
            finding("src/live.rs", CouplingKind::Common),
        ]);
        s.files
            .push(crate::metrics::testutil::make_file("src/live.rs"));
        s.files
            .push(crate::metrics::testutil::make_file("src/dormant.rs"));
        s.file_change_pairs
            .push((PathBuf::from("src/live.rs"), PathBuf::from("tests/x.rs"), 5));
        for f in ["src/live.rs", "tests/x.rs"] {
            s.commits_by_file.insert(
                PathBuf::from(f),
                (0u32..10).map(crate::snapshot::CommitId).collect(),
            );
        }
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
        assert!(acts[0].text.contains("src/live.rs"));
        assert!(acts[0].text.contains("corroborated by change history"));
        assert!(acts[1].text.contains("src/dormant.rs"));
        assert!(!acts[1].text.contains("corroborated"));
    }

    #[test]
    fn coupling_actions_capped_at_ten_keeps_path_sorted_prefix() {
        // 15 same-rung, same-count, dormant Control findings on distinct files.
        // The only tiebreak is path asc, so the surviving 10 must be the
        // lexicographically smallest 10 paths — catches a cap-before-sort bug.
        let paths: Vec<String> = (0..15).map(|i| format!("src/f{i:02}.rs")).collect();
        let findings = paths
            .iter()
            .map(|p| finding(p, CouplingKind::Control))
            .collect();
        let s = snap_with(findings);
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
        assert_eq!(acts.len(), 10);
        let mut expected = paths.clone();
        expected.sort();
        for (act, want) in acts.iter().zip(expected.iter().take(10)) {
            assert!(
                act.text.contains(want.as_str()),
                "expected {want} in {}",
                act.text
            );
        }
    }

    #[test]
    fn coupling_actions_empty_when_detection_did_not_run() {
        // Findings present but no file_metrics (AST pass didn't run — ADR-005
        // backfill). Must return empty, never fabricated actions.
        let mut s = snap_with(vec![finding("src/a.rs", CouplingKind::Common)]);
        s.file_metrics.clear();
        assert!(
            generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                &s,
                &CouplingThresholds::default()
            ))
            .is_empty()
        );
    }

    #[test]
    fn coupling_actions_advice_is_kind_specific() {
        for (kind, needle) in [
            (CouplingKind::Content, "public interface"),
            (CouplingKind::Common, "injected state"),
            (CouplingKind::Inheritance, "composition"),
            (CouplingKind::Control, "intent-revealing"),
        ] {
            let s = snap_with(vec![finding("src/a.rs", kind)]);
            let acts =
                generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                    &s,
                    &CouplingThresholds::default(),
                ));
            assert!(
                acts[0].text.contains(needle),
                "kind {kind:?}: {}",
                acts[0].text
            );
        }
    }

    #[test]
    fn inheritance_ranks_between_common_and_control() {
        let s = snap_with(vec![
            finding("src/ctrl.rs", CouplingKind::Control),
            finding("src/deep.ts", CouplingKind::Inheritance),
            finding("src/glob.rs", CouplingKind::Common),
        ]);
        let actions =
            generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                &s,
                &crate::config::CouplingThresholds::default(),
            ));
        let texts: Vec<&str> = actions.iter().map(|a| a.text.as_str()).collect();
        assert!(texts[0].contains("src/glob.rs") && texts[0].contains("worst: common"));
        assert!(texts[1].contains("src/deep.ts") && texts[1].contains("worst: inheritance"));
        assert!(texts[2].contains("src/ctrl.rs") && texts[2].contains("worst: control"));
    }

    fn fm(name: &str) -> crate::snapshot::FunctionMetrics {
        crate::snapshot::FunctionMetrics {
            responsibility: Some(crate::snapshot::ResponsibilityProvenance {
                owner_id: "module:0".into(),
                owner_label: "module".into(),
                dependencies: Vec::new(),
            }),
            is_test: false,
            name: name.to_string(),
            loc: 10,
            cyclomatic_complexity: 2,
            max_nesting_depth: 1,
        }
    }

    fn expected_group<'a>(prefix: &'static str, names: Vec<&'a str>) -> MethodGroup<'a> {
        MethodGroup {
            owner_id: "module:0",
            owner_label: "module",
            prefix,
            names,
            dependency: None,
        }
    }

    fn owned(name: &str, owner_id: &str) -> crate::snapshot::FunctionMetrics {
        crate::snapshot::FunctionMetrics {
            responsibility: Some(crate::snapshot::ResponsibilityProvenance {
                owner_id: owner_id.into(),
                // Equal labels deliberately model separate declarations of one type.
                owner_label: "Widget".into(),
                dependencies: Vec::new(),
            }),
            ..fm(name)
        }
    }

    fn field(identity: &str) -> crate::snapshot::ResponsibilityDependency {
        crate::snapshot::ResponsibilityDependency::Field {
            identity: identity.into(),
            label: format!("self.{identity}"),
        }
    }

    fn callee(identity: &str) -> crate::snapshot::ResponsibilityDependency {
        crate::snapshot::ResponsibilityDependency::Callee {
            identity: identity.into(),
            label: format!("{identity}()"),
        }
    }

    fn dependent(
        name: &str,
        dependencies: Vec<crate::snapshot::ResponsibilityDependency>,
    ) -> crate::snapshot::FunctionMetrics {
        let mut function = fm(name);
        function.responsibility.as_mut().unwrap().dependencies = dependencies;
        function
    }

    #[test]
    fn every_prefix_respects_owner_identity_including_separate_declarations() {
        for prefix in GROUPING_PREFIXES {
            let mut functions = vec![
                owned(&format!("{prefix}a"), "impl:10"),
                owned(&format!("{prefix}b"), "impl:50"),
            ];
            for function in &mut functions {
                function.responsibility.as_mut().unwrap().dependencies = vec![field("state")];
            }
            assert!(group_methods_by_prefix(&functions).is_empty(), "{prefix}");
            functions.push(functions[0].clone());
            functions[2].name = format!("{prefix}c");
            let groups = group_methods_by_prefix(&functions);
            assert_eq!(groups.len(), 1, "{prefix}");
            assert_eq!(
                groups[0].names,
                vec![format!("{prefix}a"), format!("{prefix}c")]
            );
            assert_eq!(groups[0].owner_label, "Widget");
        }
    }

    #[test]
    fn unknown_owners_and_predicates_without_shared_evidence_are_ineligible() {
        let functions = vec![
            crate::snapshot::FunctionMetrics {
                responsibility: None,
                ..fm("render_a")
            },
            crate::snapshot::FunctionMetrics {
                responsibility: None,
                ..fm("render_b")
            },
            fm("is_ready"),
            fm("is_active"),
            dependent("has_state", vec![field("state")]),
            dependent("has_cache", vec![field("cache")]),
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn predicates_need_the_same_dependency_identity_and_kind() {
        let functions = vec![
            dependent("has_field", vec![field("shared")]),
            dependent("has_callee", vec![callee("shared")]),
            dependent(
                "is_first",
                vec![crate::snapshot::ResponsibilityDependency::Field {
                    identity: "first:state".into(),
                    label: "state".into(),
                }],
            ),
            dependent(
                "is_second",
                vec![crate::snapshot::ResponsibilityDependency::Field {
                    identity: "second:state".into(),
                    label: "state".into(),
                }],
            ),
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn duplicate_dependency_entries_do_not_satisfy_group_minimum() {
        let functions = vec![
            dependent("has_a", vec![field("state"), field("state")]),
            fm("has_b"),
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn predicate_transitive_chain_selects_one_pair_and_removes_singletons() {
        let functions = vec![
            dependent("is_a", vec![field("x")]),
            dependent("is_b", vec![field("x"), field("y")]),
            dependent("is_c", vec![field("y")]),
        ];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].names, vec!["is_a", "is_b"]);
        assert_eq!(groups[0].dependency, Some(&field("x")));
    }

    #[test]
    fn largest_dependency_group_wins_then_remaining_members_can_form_a_group() {
        let functions = vec![
            dependent("has_a", vec![field("a"), field("z")]),
            dependent("has_b", vec![field("z")]),
            dependent("has_c", vec![field("z")]),
            dependent("has_d", vec![field("a")]),
            dependent("has_e", vec![field("a")]),
            dependent("has_f", vec![field("z")]),
        ];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(groups.len(), 2);
        let sharing = |name: &str| {
            groups
                .iter()
                .find(|group| group.dependency == Some(&field(name)))
                .map(|group| group.names.clone())
        };
        // has_a shares both fields; the larger `z` group claims it first.
        assert_eq!(sharing("z"), Some(vec!["has_a", "has_b", "has_c", "has_f"]));
        assert_eq!(sharing("a"), Some(vec!["has_d", "has_e"]));
        assert_eq!(
            groups.iter().map(|group| group.names.len()).sum::<usize>(),
            6
        );
    }

    #[test]
    fn overlapping_ties_prefer_fields_then_source_identity_and_ignore_input_order() {
        let mut functions = vec![
            dependent("is_a", vec![callee("a"), field("z"), field("b")]),
            dependent("is_b", vec![callee("a")]),
            dependent("is_c", vec![field("z")]),
            dependent("is_d", vec![field("b")]),
        ];
        let before: Vec<_> = group_methods_by_prefix(&functions)
            .iter()
            .map(|group| {
                (
                    group
                        .names
                        .iter()
                        .map(|name| name.to_string())
                        .collect::<Vec<_>>(),
                    group.dependency.cloned(),
                )
            })
            .collect();
        assert_eq!(
            before,
            vec![(vec!["is_a".into(), "is_d".into()], Some(field("b")))]
        );
        functions.reverse();
        for function in &mut functions {
            function
                .responsibility
                .as_mut()
                .unwrap()
                .dependencies
                .reverse();
        }
        let after: Vec<_> = group_methods_by_prefix(&functions)
            .iter()
            .map(|group| {
                (
                    group
                        .names
                        .iter()
                        .map(|name| name.to_string())
                        .collect::<Vec<_>>(),
                    group.dependency.cloned(),
                )
            })
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn recognized_tests_cannot_complete_a_predicate_group() {
        let functions = vec![
            dependent("is_ready", vec![callee("check")]),
            crate::snapshot::FunctionMetrics {
                is_test: true,
                ..dependent("is_ready_test", vec![callee("check")])
            },
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn advice_names_the_owner_and_direct_field_or_callee() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            "/tmp".into(),
            "test".into(),
            "main".into(),
            Default::default(),
        );
        snapshot.file_metrics.insert(
            "god.rs".into(),
            crate::snapshot::FileComplexity {
                functions: vec![
                    dependent("has_a", vec![field("state")]),
                    dependent("has_b", vec![field("state")]),
                    dependent("is_a", vec![callee("check")]),
                    dependent("is_b", vec![callee("check")]),
                ],
                ..Default::default()
            },
        );
        let actions =
            generate_refactoring_actions(&snapshot, &[("god.rs".into(), "520 loc".into())]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].text, "[Health] god.rs — 520 loc — consider splitting by responsibility: module: has_* (2) shared field self.state, is_* (2) shared callee check()");
        assert_eq!(actions[0].sort_by, Some(SortKey::Complexity));
    }

    fn owned_by(name: &str, owner_id: &str, owner_label: &str) -> crate::snapshot::FunctionMetrics {
        crate::snapshot::FunctionMetrics {
            responsibility: Some(crate::snapshot::ResponsibilityProvenance {
                owner_id: owner_id.into(),
                owner_label: owner_label.into(),
                dependencies: Vec::new(),
            }),
            ..fm(name)
        }
    }

    fn advice_for(functions: Vec<crate::snapshot::FunctionMetrics>) -> String {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            "/tmp".into(),
            "test".into(),
            "main".into(),
            Default::default(),
        );
        snapshot.file_metrics.insert(
            "god.rs".into(),
            crate::snapshot::FileComplexity {
                functions,
                ..Default::default()
            },
        );
        let actions =
            generate_refactoring_actions(&snapshot, &[("god.rs".into(), "520 loc".into())]);
        assert_eq!(actions.len(), 1);
        actions[0]
            .text
            .strip_prefix("[Health] god.rs — 520 loc — consider splitting by responsibility: ")
            .expect("advice prefix")
            .to_owned()
    }

    #[test]
    fn advice_orders_owners_by_source_position_and_merges_each_owners_groups() {
        // Byte offset 120 precedes 1000 in the source, though "impl_item:1000"
        // sorts first as a string.
        let advice = advice_for(vec![
            owned_by("set_a", "impl_item:120", "Early (line 5)"),
            owned_by("get_a", "impl_item:120", "Early (line 5)"),
            owned_by("get_b", "impl_item:120", "Early (line 5)"),
            owned_by("set_b", "impl_item:120", "Early (line 5)"),
            owned_by("get_c", "impl_item:1000", "Later (line 40)"),
            owned_by("get_d", "impl_item:1000", "Later (line 40)"),
        ]);

        assert_eq!(
            advice,
            "Early (line 5): get_* (2), set_* (2) | Later (line 40): get_* (2)"
        );
    }

    #[test]
    fn advice_shows_at_most_five_owners_and_counts_the_remaining_groups() {
        let functions = (0..7)
            .flat_map(|owner| {
                let id = format!("impl_item:{owner}");
                let label = format!("Owner{owner} (line {owner})");
                let mut members = vec![
                    owned_by(&format!("get_a{owner}"), &id, &label),
                    owned_by(&format!("get_b{owner}"), &id, &label),
                ];
                if owner == 5 {
                    members.push(owned_by("set_a5", &id, &label));
                    members.push(owned_by("set_b5", &id, &label));
                }
                members
            })
            .collect();

        assert_eq!(
            advice_for(functions),
            "Owner0 (line 0): get_* (2) | Owner1 (line 1): get_* (2) | Owner2 (line 2): get_* (2) | Owner3 (line 3): get_* (2) | Owner5 (line 5): get_* (2), set_* (2) | +2 more groups"
        );
    }

    fn owners_with_get_groups(sizes: &[usize]) -> Vec<crate::snapshot::FunctionMetrics> {
        sizes
            .iter()
            .enumerate()
            .flat_map(|(owner, size)| {
                let id = format!("impl_item:{owner}");
                let label = format!("Owner{owner} (line {owner})");
                (0..*size)
                    .map(|member| owned_by(&format!("get_m{owner}_{member}"), &id, &label))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn advice_keeps_the_largest_owners_even_when_they_come_last_in_source() {
        assert_eq!(
            advice_for(owners_with_get_groups(&[2, 2, 2, 2, 2, 2, 6])),
            "Owner0 (line 0): get_* (2) | Owner1 (line 1): get_* (2) | Owner2 (line 2): get_* (2) | Owner3 (line 3): get_* (2) | Owner6 (line 6): get_* (6) | +2 more groups"
        );
    }

    #[test]
    fn advice_counts_every_group_of_a_hidden_owner() {
        let mut functions = owners_with_get_groups(&[5, 5, 5, 5, 5]);
        functions.extend(
            ["get_a", "get_b", "set_a", "set_b"]
                .map(|name| owned_by(name, "impl_item:9", "Owner9 (line 9)")),
        );
        assert_eq!(
            advice_for(functions),
            "Owner0 (line 0): get_* (5) | Owner1 (line 1): get_* (5) | Owner2 (line 2): get_* (5) | Owner3 (line 3): get_* (5) | Owner4 (line 4): get_* (5) | +2 more groups"
        );
    }

    #[test]
    fn advice_names_a_single_hidden_group_in_the_singular() {
        assert_eq!(
            advice_for(owners_with_get_groups(&[2, 2, 2, 2, 2, 2])),
            "Owner0 (line 0): get_* (2) | Owner1 (line 1): get_* (2) | Owner2 (line 2): get_* (2) | Owner3 (line 3): get_* (2) | Owner4 (line 4): get_* (2) | +1 more group"
        );
    }

    #[test]
    fn prefix_matching_accepts_unicode_names_without_panicking() {
        for name in ["éé", "日", "has日", "is日", "get_日", "évalidate"] {
            let _ = group_methods_by_prefix(&[fm(name)]);
        }
        assert!(matches_group_prefix("get_日", "get_"));
        assert!(!matches_group_prefix("has日", "has_"));
    }

    #[test]
    fn pinned_barad_dur_dc23fbd6_does_not_group_unrelated_has_predicates() {
        // Exact function excerpts from dc23fbd6:src/metrics/coupling/mod.rs.
        // External helper declarations and unrelated functions are omitted.
        let source = r#"
fn has_edge(graph: &HashMap<PathBuf, Vec<PathBuf>>, from: &PathBuf, to: &PathBuf) -> bool {
    graph.get(from).is_some_and(|targets| targets.contains(to))
}
fn has_import_extractable_files(snapshot: &RepoSnapshot) -> bool {
    snapshot.files.iter().any(|file| {
        let ext = file.path.extension().and_then(|e| e.to_str()).unwrap_or("");
        import_query(detect_language(&file.path.to_string_lossy()), ext).is_some()
            && crate::collector::resolves_imports(ext)
    })
}
fn has_detectable_files(snapshot: &RepoSnapshot) -> bool {
    snapshot.files.iter().any(|f| {
        f.path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| DETECTABLE_EXTS.contains(&e))
    })
}
"#;
        let metrics =
            crate::metrics::complexity::analyse_source(Path::new("mod.rs"), source).metrics;
        assert_eq!(metrics.functions.len(), 3, "nonempty pinned extraction");
        assert!(metrics
            .functions
            .iter()
            .all(|function| function.responsibility.is_some()));
        assert!(
            group_methods_by_prefix(&metrics.functions).is_empty(),
            "the previous has_* (3) mixed unrelated predicates"
        );
    }

    #[test]
    fn pinned_ripgrep_3fce3b5b_does_not_group_eight_is_predicates_across_owners() {
        // Exact method bodies and containing declaration headers from
        // 3fce3b5b:crates/matcher/src/lib.rs. Unrelated members/docs omitted.
        let source = r#"
pub struct Match { start: usize, end: usize }
impl Match {
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
pub struct LineTerminator(LineTerminatorImp);
impl LineTerminator {
    pub fn is_crlf(&self) -> bool {
        self.0 == LineTerminatorImp::CRLF
    }
    pub fn is_suffix(&self, slice: &[u8]) -> bool {
        slice.last().map_or(false, |&b| b == self.as_byte())
    }
}
pub trait Captures {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
pub trait Matcher {
    fn is_match(&self, haystack: &[u8]) -> Result<bool, Self::Error> {
        self.is_match_at(haystack, 0)
    }
    fn is_match_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, Self::Error> {
        Ok(self.shortest_match_at(haystack, at)?.is_some())
    }
}
impl<'a, M: Matcher> Matcher for &'a M {
    fn is_match(&self, haystack: &[u8]) -> Result<bool, Self::Error> {
        (*self).is_match(haystack)
    }
    fn is_match_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, Self::Error> {
        (*self).is_match_at(haystack, at)
    }
}
"#;
        let metrics =
            crate::metrics::complexity::analyse_source(Path::new("lib.rs"), source).metrics;
        assert_eq!(
            metrics
                .functions
                .iter()
                .filter(|function| function.name.starts_with("is_"))
                .count(),
            8,
            "nonempty pinned extraction"
        );
        assert!(metrics
            .functions
            .iter()
            .all(|function| function.responsibility.is_some()));
        assert!(
            group_methods_by_prefix(&metrics.functions).is_empty(),
            "the previous is_* (8) crossed containing declarations and shared no direct dependency"
        );
    }

    /// Groups from a real extraction, as (owner label, prefix, names, evidence label).
    fn extracted_groups(
        path: &str,
        source: &str,
    ) -> Vec<(String, &'static str, Vec<String>, Option<String>)> {
        use crate::snapshot::ResponsibilityDependency;
        let metrics = crate::metrics::complexity::analyse_source(Path::new(path), source).metrics;
        assert!(!metrics.functions.is_empty(), "nonempty extraction: {path}");
        group_methods_by_prefix(&metrics.functions)
            .into_iter()
            .map(|group| {
                (
                    group.owner_label.to_owned(),
                    group.prefix,
                    group.names.iter().map(|name| (*name).to_owned()).collect(),
                    group.dependency.map(|dependency| match dependency {
                        ResponsibilityDependency::Field { label, .. }
                        | ResponsibilityDependency::Callee { label, .. } => label.clone(),
                    }),
                )
            })
            .collect()
    }

    #[test]
    fn same_named_members_of_one_owner_count_once() {
        let accessor =
            "class A { #r; get isReady(){ return this.#r } set isReady(v){ this.#r = v } }";
        assert_eq!(extracted_groups("src/a.js", accessor), vec![]);
        let overloads = "class A { Object state; boolean isValid() { return state != null; } boolean isValid(int limit) { return state != null && limit > 0; } }";
        assert_eq!(extracted_groups("src/A.java", overloads), vec![]);
        let with_sibling = "class A { #r; get isReady(){ return this.#r } set isReady(v){ this.#r = v } isReadyLater(){ return this.#r } }";
        assert_eq!(
            extracted_groups("src/a.js", with_sibling),
            vec![(
                "A (line 1)".to_owned(),
                "is_",
                vec!["isReady".to_owned(), "isReadyLater".to_owned()],
                Some("#r".to_owned()),
            )]
        );
    }

    #[test]
    fn a_local_named_like_a_field_does_not_complete_a_field_group() {
        let source = "class Cache { string value; Dictionary<string,string> store; bool IsLoaded() => value != null; bool IsFresh(string k) => store.TryGetValue(k, out var value) && value != null; }";
        assert_eq!(extracted_groups("src/Cache.cs", source), vec![]);
    }

    #[test]
    fn a_call_with_a_different_argument_count_is_not_shared_callee_evidence() {
        let mismatched = "class Widget extends Base { boolean ready; boolean check(){ return ready; } boolean isA(){ return check(); } boolean isB(){ return check(2); } }";
        assert_eq!(extracted_groups("src/Widget.java", mismatched), vec![]);
        let matched = "class Widget extends Base { boolean ready; boolean check(){ return ready; } boolean isA(){ return check(); } boolean isB(){ return check(); } }";
        assert_eq!(
            extracted_groups("src/Widget.java", matched),
            vec![(
                "Widget (line 1)".to_owned(),
                "is_",
                vec!["isA".to_owned(), "isB".to_owned()],
                Some("check".to_owned()),
            )]
        );
    }

    #[test]
    fn tagged_template_calls_are_not_shared_callee_evidence() {
        let source = "class A { tag(s){ return s } isA(){ return this.tag`${x}`; } isB(){ return this.tag`${y}`; } }";
        assert_eq!(extracted_groups("src/a.js", source), vec![]);
    }

    #[test]
    fn a_recursive_call_is_not_evidence_shared_with_its_caller() {
        let source = "fn is_sorted(v:&[i32])->bool{ v.len()<2 || (v[0]<=v[1] && is_sorted(&v[1..])) }\nfn is_strictly_sorted(v:&[i32])->bool{ is_sorted(v) && v.windows(2).all(|w| w[0]!=w[1]) }\n";
        assert_eq!(extracted_groups("src/lib.rs", source), vec![]);
    }

    #[test]
    fn go_methods_keep_their_group_when_the_receiver_type_lives_in_another_file() {
        let source = "package api\nfunc (s *Server) handleA() {}\nfunc (s *Server) handleB() {}\nfunc (s *Server) handleC() {}\n";
        assert_eq!(
            extracted_groups("src/handlers.go", source),
            vec![(
                "*Server".to_owned(),
                "handle_",
                vec![
                    "handleA".to_owned(),
                    "handleB".to_owned(),
                    "handleC".to_owned()
                ],
                None,
            )]
        );
    }

    #[test]
    fn test_functions_are_removed_before_group_minimum_counts_and_ranking() {
        let test = |name: &str| crate::snapshot::FunctionMetrics {
            is_test: true,
            ..fm(name)
        };
        let functions = vec![
            fm("render_a"),
            fm("render_b"),
            test("render_case"),
            fm("build_a"),
            test("build_case"),
        ];
        assert_eq!(
            group_methods_by_prefix(&functions),
            vec![expected_group("render_", vec!["render_a", "render_b"])]
        );
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            "/tmp".into(),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        for (path, count) in [
            ("a.rs", 2),
            ("b.rs", 3),
            ("c.rs", 4),
            ("d.rs", 5),
            ("e.rs", 6),
            ("f.rs", 7),
        ] {
            let mut functions: Vec<_> = (0..count).map(|i| fm(&format!("render_{i}"))).collect();
            if path == "a.rs" {
                functions.extend((0..50).map(|i| test(&format!("render_test_{i}"))));
                functions.extend((0..50).map(|i| fm(&format!("has_unknown_{i}"))));
                functions.extend((0..50).map(|i| crate::snapshot::FunctionMetrics {
                    responsibility: None,
                    ..fm(&format!("build_unknown_{i}"))
                }));
            }
            snapshot.file_metrics.insert(
                path.into(),
                crate::snapshot::FileComplexity {
                    loc: 520,
                    cyclomatic_complexity: 1,
                    functions,
                    ..Default::default()
                },
            );
        }
        snapshot.file_metrics.insert(
            "all.rs".into(),
            crate::snapshot::FileComplexity {
                loc: 520,
                cyclomatic_complexity: 1,
                functions: (0..100).map(|i| test(&format!("render_{i}"))).collect(),
                ..Default::default()
            },
        );
        let flagged = crate::metrics::health::god_object_files(
            &snapshot,
            &crate::config::HealthThresholds::default(),
        );
        assert_eq!(flagged.len(), 7);
        let actions = generate_refactoring_actions(&snapshot, &flagged);
        assert_eq!(actions.len(), 5);
        for (action, path) in actions.iter().zip(["f.rs", "e.rs", "d.rs", "c.rs", "b.rs"]) {
            assert!(action.text.contains(path), "{}", action.text);
        }
    }

    #[test]
    fn group_methods_by_prefix_groups_shared_verbs() {
        let functions = vec![
            fm("handle_a"),
            fm("handle_b"),
            fm("handle_c"),
            fm("validate_x"),
            fm("validate_y"),
            fm("parse_one"),
        ];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![
                expected_group("handle_", vec!["handle_a", "handle_b", "handle_c"]),
                expected_group("validate_", vec!["validate_x", "validate_y"]),
            ]
        );
    }

    #[test]
    fn group_methods_by_prefix_excludes_singleton_groups() {
        let functions = vec![fm("parse_only_one"), fm("main")];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn group_methods_by_prefix_returns_empty_for_no_matches() {
        let functions = vec![fm("run")];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn group_methods_by_prefix_matches_camel_case_boundaries() {
        // The collector supports 8 languages via tree-sitter; camelCase
        // methods (Java/C#/JS/TS/Kotlin/Swift) must cluster exactly like
        // their snake_case equivalents.
        let functions = vec![fm("getUserData"), fm("getUserProfile"), fm("handleClick")];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![expected_group(
                "get_",
                vec!["getUserData", "getUserProfile"]
            )]
        );
    }

    #[test]
    fn group_methods_by_prefix_camel_case_boundary_excludes_lookalikes() {
        // "getter" and "geta" share the "get" letters but not a real word
        // boundary (next char is lowercase, neither '_' nor uppercase) —
        // must not match, same discipline as the snake_case case.
        let functions = vec![fm("getter"), fm("geta")];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn group_methods_by_prefix_mixed_snake_and_camel_case_share_a_group() {
        let functions = vec![fm("handle_click"), fm("handleSubmit")];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![expected_group(
                "handle_",
                vec!["handleSubmit", "handle_click"]
            )]
        );
    }

    #[test]
    fn group_methods_by_prefix_matches_pascal_case_exported_methods() {
        // Go and C# capitalize exported/public method names (GetUserData,
        // not getUserData) — the verb itself must match case-insensitively,
        // with the boundary check (next char uppercase) unchanged.
        let functions = vec![fm("GetUserData"), fm("GetUserProfile"), fm("HandleClick")];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![expected_group(
                "get_",
                vec!["GetUserData", "GetUserProfile"]
            )]
        );
    }

    #[test]
    fn generate_refactoring_actions_emits_action_for_clustering_god_object() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            std::path::PathBuf::from("god.rs"),
            crate::snapshot::FileComplexity {
                total_lines: 600,
                loc: 520,
                cyclomatic_complexity: 10,
                public_methods: 5,
                properties: 2,
                functions: vec![fm("handle_a"), fm("handle_b"), fm("main")],
                ..Default::default()
            },
        );
        let thresholds = crate::config::HealthThresholds::default();
        let actions = generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds),
        );
        assert_eq!(actions.len(), 1);
        assert!(actions[0].text.contains("god.rs"));
        assert!(actions[0].text.contains("520 loc"));
        assert!(actions[0].text.contains("handle_* (2)"));
        assert_eq!(actions[0].target_tab, Some(ReportTab::Hotspots));
    }

    #[test]
    fn generate_refactoring_actions_ranks_by_group_size_and_caps_at_five() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        // 6 god-object files, each with a distinct total clustering-method
        // count (sum of group sizes): f0=2 (smallest), f5=7 (largest). Only
        // the top 5 by count should survive, in descending order; f0 (count
        // 2, the smallest) must be dropped.
        let counts = [
            ("f0.rs", 2),
            ("f1.rs", 3),
            ("f2.rs", 4),
            ("f3.rs", 5),
            ("f4.rs", 6),
            ("f5.rs", 7),
        ];
        for (name, count) in counts {
            let functions: Vec<_> = (0..count).map(|i| fm(&format!("handle_{i}"))).collect();
            snapshot.file_metrics.insert(
                std::path::PathBuf::from(name),
                crate::snapshot::FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 10,
                    public_methods: 5,
                    properties: 2,
                    functions,
                    ..Default::default()
                },
            );
        }
        let thresholds = crate::config::HealthThresholds::default();
        let actions = generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds),
        );
        assert_eq!(actions.len(), 5, "must be capped at 5: {actions:#?}");
        let expected_order = ["f5.rs", "f4.rs", "f3.rs", "f2.rs", "f1.rs"];
        for (action, expected) in actions.iter().zip(expected_order.iter()) {
            assert!(
                action.text.contains(expected),
                "expected {expected} next, got: {}",
                action.text
            );
        }
        assert!(
            actions.iter().all(|a| !a.text.contains("f0.rs")),
            "smallest group (f0.rs) must be dropped by the cap"
        );
    }

    #[test]
    fn generate_refactoring_actions_tie_break_sorts_by_display_string_not_pathbuf() {
        // Regression guard mirroring god_object_files' own fix: PathBuf::cmp
        // compares path COMPONENTS, which disagrees with plain byte-string
        // comparison for a pair like "src-utils.rs" (one component) vs
        // "src/utils.rs" (two components) — '-' (0x2D) < '/' (0x2F) as
        // bytes, but "src" < "src-utils.rs" as path components. Both
        // candidates here have the same clustering-method count (2), so the
        // tie-break alone decides the order.
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        for name in ["src/utils.rs", "src-utils.rs"] {
            snapshot.file_metrics.insert(
                std::path::PathBuf::from(name),
                crate::snapshot::FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 10,
                    public_methods: 5,
                    properties: 2,
                    functions: vec![fm("handle_a"), fm("handle_b")],
                    ..Default::default()
                },
            );
        }
        let thresholds = crate::config::HealthThresholds::default();
        let actions = generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds),
        );
        assert_eq!(actions.len(), 2);
        assert!(
            actions[0].text.contains("src-utils.rs"),
            "tie-break must sort by display string ('-' < '/'), got: {:#?}",
            actions
        );
        assert!(actions[1].text.contains("src/utils.rs"));
    }

    #[test]
    fn generate_refactoring_actions_skips_god_object_with_no_clustering() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            std::path::PathBuf::from("god.rs"),
            crate::snapshot::FileComplexity {
                total_lines: 600,
                loc: 520,
                cyclomatic_complexity: 10,
                public_methods: 5,
                properties: 2,
                functions: vec![fm("run")],
                ..Default::default()
            },
        );
        let thresholds = crate::config::HealthThresholds::default();
        assert!(generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds)
        )
        .is_empty());
    }

    #[test]
    fn generate_refactoring_actions_skips_non_god_object_with_clustering_names() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        // Small file (not flagged as a god object) with clustering method names —
        // proves the shared-selection-function gate (decision 6 in the spec).
        snapshot.file_metrics.insert(
            std::path::PathBuf::from("small.rs"),
            crate::snapshot::FileComplexity {
                total_lines: 50,
                loc: 40,
                cyclomatic_complexity: 3,
                public_methods: 2,
                properties: 1,
                functions: vec![fm("handle_a"), fm("handle_b")],
                ..Default::default()
            },
        );
        let thresholds = crate::config::HealthThresholds::default();
        assert!(generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds)
        )
        .is_empty());
    }
}
