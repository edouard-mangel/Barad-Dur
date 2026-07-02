mod analyze;
pub use analyze::AnalyzeArgs;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "barad-dur",
    version,
    about = "The all-seeing repository analyzer",
    long_about = "The all-seeing repository analyzer.\n\n\
        Barad-dur analyzes git metadata (commits, blame, file tree) and source code \
        complexity to produce a scored report across 5 categories: Health, Coupling, \
        Evolution, Git Hygiene, and Team. Each metric scores 0-100 and the report includes \
        actionable recommendations from the lowest-scoring metrics.\n\n\
        Supports local paths and remote URLs. When given a URL, the repository is \
        cloned into a temporary directory and cleaned up after analysis.",
    after_long_help = "EXAMPLES:\n    \
        barad-dur analyze .                              # analyze current repo\n    \
        barad-dur analyze . -v                           # show individual metrics\n    \
        barad-dur analyze . --json --pretty -o report.json\n    \
        barad-dur analyze . --html -o report.html\n    \
        barad-dur analyze . --open                       # analyze + open in browser\n    \
        barad-dur analyze . --health --team              # specific categories\n    \
        barad-dur analyze . --since 3months\n    \
        barad-dur analyze https://github.com/user/repo --token ghp_xxx"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze a git repository
    Analyze(AnalyzeArgs),
    /// Backfill historical trend entries for a git repository
    Backfill(BackfillArgs),
    /// Generate a .repository-analysis/barad-dur.toml configuration file
    Init(InitArgs),
    /// Quality gate — exit non-zero if score is below threshold
    Gate(GateArgs),
    /// Analyze cross-repository coupling (temporal, team, dependency)
    Coupling(CouplingArgs),
    /// Install or remove a post-commit git hook that re-runs analysis on each commit
    Watch(WatchArgs),
    /// Detect suspected duplicate contributors and suggest .mailmap entries
    Contributors(ContributorsArgs),
}

#[derive(clap::Args, Debug)]
#[command(
    about = "Backfill historical trend entries for a git repository",
    long_about = "Walks the commit history and samples representative snapshots to populate \
        trends.json with historical analysis data. Uses adaptive sampling to select up to \
        the configured number of commits (default: 10)."
)]
pub struct BackfillArgs {
    /// Path to the git repository
    #[arg(default_value = ".")]
    pub target: String,

    /// Skip git blame during backfill (faster but omits blame-dependent metrics)
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub no_blame: bool,
}

#[derive(clap::Args, Debug)]
#[command(
    about = "Generate a .repository-analysis/barad-dur.toml config file with smart defaults",
    long_about = "Scans the repository to detect translation files, generated code, \
        vendored dependencies, and team patterns, then generates a commented config file \
        with recommended settings. Detected exclusion patterns are written to a \
        .baraddurignore file at the repo root (full .gitignore syntax), not to the TOML.\n\n\
        Use --interactive for a guided wizard that walks through each setting."
)]
pub struct InitArgs {
    /// Path to the git repository
    #[arg(default_value = ".")]
    pub target: String,

    /// Run interactive wizard instead of auto-detecting
    #[arg(short, long)]
    pub interactive: bool,

    /// Overwrite existing config file
    #[arg(long)]
    pub force: bool,
}

#[derive(clap::Args, Debug)]
#[command(
    about = "Quality gate — exit non-zero if score is below threshold",
    long_about = "Runs analysis and checks whether the overall score (or per-category scores) \
        meet a minimum threshold. Exits with code 0 if the gate passes, or code 1 if any \
        checked score falls below the threshold.\n\n\
        Designed for CI/CD pipelines. Uses cached data when available.",
    after_long_help = "\
EXAMPLES:\n    \
  barad-dur gate .                         # default: overall >= 60\n    \
  barad-dur gate . --min-score 70          # overall >= 70\n    \
  barad-dur gate . --category health       # check health category only\n    \
  barad-dur gate . --category health --category team  # check both"
)]
pub struct GateArgs {
    /// Path to the git repository
    #[arg(default_value = ".")]
    pub target: String,

    /// Minimum score to pass (0-100)
    #[arg(long, default_value = "60")]
    pub min_score: u32,

    /// Check specific category scores instead of overall
    ///
    /// When specified, each named category must individually meet --min-score.
    /// Can be repeated: --category health --category team
    #[arg(long, action = clap::ArgAction::Append)]
    pub category: Vec<String>,

    /// Skip git blame for faster checks (blame-dependent metrics get defaults)
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub skip_blame: Option<bool>,

    /// Fail if the score is declining faster than this many points per run
    ///
    /// Compares the current score against prior runs on the same branch.
    /// A value of 2.0 means: fail if the score drops more than 2 points per run
    /// on average over the last 8 runs. Set to 0 to fail on any decline.
    #[arg(long)]
    pub max_decline: Option<f64>,
}

#[derive(clap::Args, Debug)]
#[command(
    about = "Analyze cross-repository coupling (temporal, team, dependency)",
    long_about = "Discovers git repositories under a root directory and analyzes coupling \
        signals between them: temporal (commits within a time window), team (shared \
        contributors), and dependency (shared libraries/packages).\n\n\
        Produces a scored report of repository pairs ranked by combined coupling strength.",
    after_long_help = "\
EXAMPLES:\n    \
  barad-dur coupling /path/to/workspace          # analyze all repos under workspace\n    \
  barad-dur coupling . --json                     # JSON output\n    \
  barad-dur coupling . --min-score 50             # only show pairs scoring >= 50\n    \
  barad-dur coupling . --coupling-window 12h      # 12-hour coupling window\n    \
  barad-dur coupling . --since 3months            # limit to last 3 months"
)]
pub struct CouplingArgs {
    /// Root directory containing multiple git repositories
    pub root_dir: PathBuf,

    /// Maximum time window for commits to be considered coupled [default: 24h]
    ///
    /// Commits in two different repos that fall within this window are
    /// counted as temporally coupled. Accepts durations like "24h", "12h", "48h".
    #[arg(long, default_value = "24h")]
    pub coupling_window: String,

    /// Start of analysis window (how far back to look)
    ///
    /// Accepts relative durations (3months, 30days, 1year) or
    /// ISO dates (2024-01-01). Defaults to 6 months ago.
    #[arg(long)]
    pub since: Option<String>,

    /// Minimum combined score to include a pair in the report (0-100)
    #[arg(long, default_value = "30.0")]
    pub min_score: f64,

    /// Output as JSON instead of CLI table
    #[arg(long)]
    pub json: bool,

    /// Pretty-print JSON output (only effective with --json)
    #[arg(long)]
    pub pretty: bool,

    /// Output as self-contained HTML report
    #[arg(long)]
    pub html: bool,

    /// Generate HTML report and open it in the default browser
    #[arg(long)]
    pub open: bool,

    /// Write output to a file instead of stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Increase verbosity (-v shows details, -vv shows raw data)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(clap::Args, Debug)]
#[command(
    about = "Install or remove a post-commit git hook that re-runs analysis on each commit",
    long_about = "Installs a post-commit git hook that runs `barad-dur analyze` after every \
        commit and prints a delta summary (score change, changed categories). Use --uninstall \
        to remove the hook.\n\n\
        The hook is written to .git/hooks/post-commit. If a hook already exists it will not \
        be overwritten unless --force is supplied.",
    after_long_help = "\
EXAMPLES:\n    \
  barad-dur watch .                  # install hook in current repo\n    \
  barad-dur watch . --skip-blame     # faster hook (no blame phase)\n    \
  barad-dur watch . --uninstall      # remove the hook"
)]
pub struct WatchArgs {
    /// Path to the git repository
    #[arg(default_value = ".")]
    pub target: String,

    /// Remove the installed hook instead of installing it
    #[arg(long)]
    pub uninstall: bool,

    /// Overwrite an existing post-commit hook
    #[arg(long)]
    pub force: bool,

    /// Pass --skip-blame to the hook for faster analysis
    #[arg(long)]
    pub skip_blame: bool,
}

#[derive(clap::Args, Debug)]
#[command(about = "Detect suspected duplicate contributors and suggest .mailmap entries")]
pub struct ContributorsArgs {
    /// Path to the git repository
    #[arg(default_value = ".")]
    pub target: String,

    /// Append suggested entries to .mailmap (creates it if absent)
    #[arg(long)]
    pub write: bool,

    /// Only consider commits in this time window (e.g. 6months, 1year)
    #[arg(long)]
    pub since: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse_gate(args: &[&str]) -> GateArgs {
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Gate(a) => a,
            _ => panic!("expected Gate command"),
        }
    }

    #[test]
    fn gate_default_args() {
        let args = parse_gate(&["barad-dur", "gate", "."]);
        assert_eq!(args.target, ".");
        assert_eq!(args.min_score, 60);
        assert!(args.category.is_empty());
    }

    #[test]
    fn gate_min_score() {
        let args = parse_gate(&["barad-dur", "gate", ".", "--min-score", "75"]);
        assert_eq!(args.min_score, 75);
    }

    #[test]
    fn gate_category_filter() {
        let args = parse_gate(&[
            "barad-dur",
            "gate",
            ".",
            "--category",
            "health",
            "--category",
            "team",
        ]);
        assert_eq!(args.category, vec!["health", "team"]);
    }

    #[test]
    fn gate_max_decline_flag() {
        let args = parse_gate(&["barad-dur", "gate", ".", "--max-decline", "3.5"]);
        assert_eq!(args.max_decline, Some(3.5));
    }

    #[test]
    fn gate_max_decline_absent() {
        let args = parse_gate(&["barad-dur", "gate", "."]);
        assert_eq!(args.max_decline, None);
    }

    #[test]
    fn init_subcommand() {
        let cli = Cli::parse_from(["barad-dur", "init"]);
        assert!(matches!(cli.command, Commands::Init(_)));
    }

    #[test]
    fn init_interactive_flag() {
        let cli = Cli::parse_from(["barad-dur", "init", "-i"]);
        match cli.command {
            Commands::Init(args) => assert!(args.interactive),
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn init_force_flag() {
        let cli = Cli::parse_from(["barad-dur", "init", "--force"]);
        match cli.command {
            Commands::Init(args) => assert!(args.force),
            _ => panic!("expected Init"),
        }
    }
}
