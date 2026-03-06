use barad_dur::cli::{Cli, Commands};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => {
            println!("{:?}", args);
        }
    }
}
