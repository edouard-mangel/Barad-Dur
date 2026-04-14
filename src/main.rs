use anyhow::Result;
use clap::Parser;

use barad_dur::backfill;
use barad_dur::cli::{Cli, Commands};
use barad_dur::cmd::{analyze, coupling, gate};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => analyze::run_analyze(args)?,
        Commands::Backfill(args) => {
            let repo_path = std::path::PathBuf::from(&args.target);
            backfill::run(&args, &repo_path)?;
        }
        Commands::Init(args) => {
            let target = std::path::PathBuf::from(&args.target);
            barad_dur::init::run_init(&target, args.force, args.interactive)?;
        }
        Commands::Gate(args) => {
            std::process::exit(gate::run_gate(args)?);
        }
        Commands::Coupling(args) => coupling::run_coupling(args)?,
    }
    Ok(())
}
