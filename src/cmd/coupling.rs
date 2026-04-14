use anyhow::Result;
use std::io::IsTerminal;
use std::path::PathBuf;

use crate::cli::CouplingArgs;
use crate::coupling::collector::{collect_snapshots, CollectionResult};
use crate::coupling::dependency::{analyze_dependency_coupling, DependencyAnalysis};
use crate::coupling::discovery::discover_repos;
use crate::coupling::scorer::score_coupling_pairs;
use crate::coupling::team::analyze_team_coupling;
use crate::coupling::temporal::{analyze_temporal_coupling, TemporalCouplingPair};
use crate::coupling::types::CouplingPair;
use crate::coupling::{CouplingReport, CouplingReportSummary, RepoInfo};
use crate::renderer;
use crate::renderer::coupling_cli::render_coupling_table;
use crate::renderer::coupling_json::render_coupling_json;
use crate::runner;
use indicatif::{ProgressBar, ProgressStyle};

pub fn run_coupling(args: CouplingArgs) -> Result<()> {
    let is_tty = std::io::stderr().is_terminal();

    let make_spinner = |msg: &str| -> ProgressBar {
        if !is_tty {
            return ProgressBar::hidden();
        }
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.cyan} {msg}")
                .unwrap(),
        );
        sp.set_message(msg.to_string());
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        sp
    };

    // Step 1: Discover repos under root directory
    let sp = make_spinner("Discovering repositories...");
    let discovery = discover_repos(&args.root_dir);
    sp.finish_with_message(format!(
        "Discovered {} repos (skipped {})",
        discovery.discovered.len(),
        discovery.skipped.len()
    ));

    if discovery.discovered.len() < 2 {
        eprintln!(
            "Found {} repos under {}. Need at least 2 for coupling analysis.",
            discovery.discovered.len(),
            args.root_dir.display()
        );
        return Ok(());
    }

    // Step 2: Collect snapshots (skip-blame, parallel)
    let sp = make_spinner(&format!(
        "Collecting snapshots from {} repos...",
        discovery.discovered.len()
    ));
    let config = crate::coupling::CouplingConfig {
        root_dir: args.root_dir.clone(),
        ..Default::default()
    };
    let collection = collect_snapshots(&discovery.discovered, &config);
    sp.finish_with_message(format!(
        "Collected {} snapshots ({} failed)",
        collection.snapshots.len(),
        collection.failed.len()
    ));

    // Step 3: Analyze all three coupling dimensions
    let sp = make_spinner("Analyzing temporal coupling...");
    let window = std::time::Duration::from_secs(24 * 60 * 60);
    let temporal_pairs = analyze_temporal_coupling(&collection.snapshots, window);
    sp.finish_with_message(format!("Temporal: {} coupled pairs", temporal_pairs.len()));

    let sp = make_spinner("Analyzing team coupling...");
    let team_pairs = analyze_team_coupling(&collection.snapshots);
    sp.finish_with_message(format!("Team: {} pairs", team_pairs.len()));

    let sp = make_spinner("Analyzing dependency coupling...");
    let repo_paths: Vec<(String, std::path::PathBuf)> = collection
        .snapshots
        .iter()
        .map(|(name, snap)| (name.clone(), snap.path.clone()))
        .collect();
    let dep_analysis = analyze_dependency_coupling(&repo_paths);
    sp.finish_with_message(format!("Dependencies: {} pairs", dep_analysis.pairs.len()));

    // Step 4: Combine scores from all dimensions
    let sp = make_spinner("Computing combined scores...");
    let combined_pairs = score_coupling_pairs(&temporal_pairs, &team_pairs, &dep_analysis);
    sp.finish_with_message(format!("Scored {} pairs", combined_pairs.len()));

    // Step 5: Render output
    render_coupling_output(
        &args,
        &collection,
        &combined_pairs,
        &dep_analysis,
        &temporal_pairs,
    )?;

    Ok(())
}

fn render_coupling_output(
    args: &CouplingArgs,
    collection: &CollectionResult,
    combined_pairs: &[CouplingPair],
    dep_analysis: &DependencyAnalysis,
    temporal_pairs: &[TemporalCouplingPair],
) -> Result<()> {
    let use_html = args.html || args.open;
    if args.json || use_html {
        let repos: Vec<RepoInfo> = collection
            .snapshots
            .iter()
            .map(|(name, snap)| RepoInfo {
                name: name.clone(),
                path: snap.path.clone(),
                commit_count: snap.commit_count,
                author_count: snap.author_count,
            })
            .collect();

        let highest = combined_pairs
            .first()
            .map(|p| p.combined_score)
            .unwrap_or(0.0);

        let pairs_above = combined_pairs
            .iter()
            .filter(|p| p.combined_score >= args.min_score)
            .count();

        let report = CouplingReport {
            repos: repos.clone(),
            pairs: combined_pairs.to_vec(),
            summary: CouplingReportSummary {
                total_repos: repos.len(),
                total_pairs_analyzed: repos.len() * (repos.len().saturating_sub(1)) / 2,
                pairs_above_threshold: pairs_above,
                highest_coupling_score: highest,
            },
            blast_radius: dep_analysis.blast_radius.clone(),
        };

        if use_html {
            let output = renderer::coupling_html::render_coupling_html(&report);
            let path = if let Some(ref p) = args.output {
                std::fs::write(p, &output)?;
                p.clone()
            } else {
                let default_path = PathBuf::from("coupling-report.html");
                std::fs::write(&default_path, &output)?;
                default_path
            };
            eprintln!("Report written to {}", path.display());
            if args.open {
                runner::open_in_browser(&path)?;
            }
        } else {
            let output = render_coupling_json(&report, args.pretty);
            if let Some(path) = &args.output {
                std::fs::write(path, &output)?;
                eprintln!("Report written to {}", path.display());
            } else {
                print!("{}", output);
            }
        }
    } else {
        let output = render_coupling_table(temporal_pairs);
        if let Some(path) = &args.output {
            std::fs::write(path, &output)?;
            eprintln!("Report written to {}", path.display());
        } else {
            print!("{}", output);
        }
    }

    Ok(())
}
