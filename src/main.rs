use anyhow::Result;
use clap::Parser;

use barad_dur::cli::{Cli, Commands};
use barad_dur::cmd::{analyze, backfill, contributors, coupling, gate, init, watch};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => analyze::run_analyze(args)?,
        Commands::Backfill(args) => backfill::run_backfill(args)?,
        Commands::Init(args) => init::run_init(args)?,
        Commands::Gate(args) => {
            std::process::exit(gate::run_gate(args)?);
        }
        Commands::Coupling(args) => coupling::run_coupling(args)?,
        Commands::Watch(args) => watch::run_watch(args)?,
        Commands::Contributors(args) => contributors::run_contributors(args)?,
    }
    Ok(())
}
