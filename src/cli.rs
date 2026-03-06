use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "barad-dur", about = "The all-seeing repository analyzer")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze a git repository
    Analyze(AnalyzeArgs),
}

#[derive(clap::Args, Debug)]
pub struct AnalyzeArgs {
    /// Path to the git repository (default: current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Run health metrics
    #[arg(long)]
    pub health: bool,

    /// Run team metrics
    #[arg(long)]
    pub team: bool,

    /// Run evolution metrics
    #[arg(long)]
    pub evolution: bool,

    /// Run git hygiene metrics
    #[arg(long)]
    pub hygiene: bool,

    /// Start of analysis window (e.g., '3months', '2024-01-01')
    #[arg(long)]
    pub since: Option<String>,

    /// End of analysis window (e.g., '2024-06-30')
    #[arg(long)]
    pub until: Option<String>,

    /// Analyze full history
    #[arg(long)]
    pub all: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Pretty-print JSON output
    #[arg(long)]
    pub pretty: bool,

    /// Write output to file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Increase verbosity (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Skip cache, force full re-collection
    #[arg(long)]
    pub no_cache: bool,

    /// Only use cache, fail if none exists
    #[arg(long)]
    pub cache_only: bool,
}

impl AnalyzeArgs {
    /// Returns true if no specific category was selected (meaning run all).
    pub fn all_categories(&self) -> bool {
        !self.health && !self.team && !self.evolution && !self.hygiene
    }

    /// Returns true if the given category should be run.
    pub fn should_run(&self, category: &str) -> bool {
        if self.all_categories() {
            return true;
        }
        match category {
            "health" => self.health,
            "team" => self.team,
            "evolution" => self.evolution,
            "hygiene" => self.hygiene,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> AnalyzeArgs {
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Analyze(a) => a,
        }
    }

    #[test]
    fn default_args() {
        let args = parse(&["barad-dur", "analyze", "."]);
        assert_eq!(args.path, PathBuf::from("."));
        assert!(args.all_categories());
        assert!(!args.json);
        assert!(!args.no_cache);
        assert_eq!(args.verbose, 0);
    }

    #[test]
    fn category_flag_health_only() {
        let args = parse(&["barad-dur", "analyze", ".", "--health"]);
        assert!(args.health);
        assert!(!args.team);
        assert!(!args.all_categories());
        assert!(args.should_run("health"));
        assert!(!args.should_run("team"));
    }

    #[test]
    fn since_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--since", "3months"]);
        assert_eq!(args.since, Some("3months".to_string()));
    }

    #[test]
    fn date_range_flags() {
        let args = parse(&[
            "barad-dur",
            "analyze",
            ".",
            "--since",
            "2024-01-01",
            "--until",
            "2024-06-30",
        ]);
        assert_eq!(args.since, Some("2024-01-01".to_string()));
        assert_eq!(args.until, Some("2024-06-30".to_string()));
    }

    #[test]
    fn all_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--all"]);
        assert!(args.all);
    }

    #[test]
    fn json_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--json"]);
        assert!(args.json);
    }

    #[test]
    fn json_pretty_flags() {
        let args = parse(&["barad-dur", "analyze", ".", "--json", "--pretty"]);
        assert!(args.json);
        assert!(args.pretty);
    }

    #[test]
    fn output_file() {
        let args = parse(&["barad-dur", "analyze", ".", "-o", "report.json"]);
        assert_eq!(args.output, Some(PathBuf::from("report.json")));
    }

    #[test]
    fn verbosity_single() {
        let args = parse(&["barad-dur", "analyze", ".", "-v"]);
        assert_eq!(args.verbose, 1);
    }

    #[test]
    fn verbosity_double() {
        let args = parse(&["barad-dur", "analyze", ".", "-vv"]);
        assert_eq!(args.verbose, 2);
    }

    #[test]
    fn no_cache_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--no-cache"]);
        assert!(args.no_cache);
    }

    #[test]
    fn cache_only_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--cache-only"]);
        assert!(args.cache_only);
    }

    #[test]
    fn all_categories_when_none_selected() {
        let args = parse(&["barad-dur", "analyze", "."]);
        assert!(args.should_run("health"));
        assert!(args.should_run("team"));
        assert!(args.should_run("evolution"));
        assert!(args.should_run("hygiene"));
    }
}
