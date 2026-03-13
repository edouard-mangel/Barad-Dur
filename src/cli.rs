use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "barad-dur",
    version,
    about = "The all-seeing repository analyzer",
    long_about = "The all-seeing repository analyzer.\n\n\
        Barad-dur analyzes git metadata (commits, blame, file tree) and source code \
        complexity to produce a scored report across 4 categories: Health, Team, \
        Evolution, and Git Hygiene. Each metric scores 0-100 and the report includes \
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
}

#[derive(clap::Args, Debug)]
#[command(
    about = "Analyze a git repository",
    long_about = "Analyze a git repository for health, team dynamics, evolution patterns, \
        and git hygiene.\n\n\
        By default, all 4 categories are computed over the last 6 months. Use category \
        flags to run a subset, and --since/--until/--all to control the time window.\n\n\
        Output defaults to a colored CLI report. Use --json for machine consumption \
        or --html for an interactive single-file report with charts and tables.",
    after_long_help = "\
METRICS:\n\
  Health (30%)      Bus factor, churn hotspots, temporal coupling, stale code, file complexity\n\
  Team (30%)        Knowledge distribution (Gini), contributor activity, ownership, silos, merges\n\
  Evolution (20%)   Growth trend, refactoring ratio, code age, commit cadence\n\
  Git Hygiene (20%) Commit message quality, history cleanliness, gitignore coverage\n\
\n\
TIME WINDOW FORMATS:\n\
  Relative:  3months, 6months, 30days, 1year\n\
  Absolute:  2024-01-01 (ISO 8601 date)\n\
\n\
EXAMPLES:\n    \
  barad-dur analyze .                                 # all categories, last 6 months\n    \
  barad-dur analyze . -v                              # show per-metric scores\n    \
  barad-dur analyze . -vv                             # also show raw values\n    \
  barad-dur analyze . --json --pretty                 # pretty-printed JSON\n    \
  barad-dur analyze . --html -o report.html           # interactive HTML report\n    \
  barad-dur analyze . --health --team                 # only Health + Team\n    \
  barad-dur analyze . --since 3months                 # custom time window\n    \
  barad-dur analyze . --since 2024-01-01 --until 2024-12-31\n    \
  barad-dur analyze . --all                           # full history\n    \
  barad-dur analyze https://github.com/user/repo      # remote repository\n    \
  barad-dur analyze https://github.com/user/repo --token ghp_xxx  # with GitHub API data"
)]
pub struct AnalyzeArgs {
    /// Path or URL to the git repository
    ///
    /// Accepts local paths and remote URLs (https://, http://, git@).
    /// Remote repositories are cloned to a temp directory and cleaned up
    /// after analysis.
    #[arg(default_value = ".")]
    pub target: String,

    /// GitHub personal access token for API enrichment
    ///
    /// When the target is a GitHub URL, enriches the report with stars,
    /// description, primary language, and open issues count.
    /// Requires at least public_repo scope (or repo for private repos).
    #[arg(long, help_heading = "Remote")]
    pub token: Option<String>,

    /// Run only the Health category (bus factor, churn, coupling, staleness, complexity)
    #[arg(long, help_heading = "Category Filters")]
    pub health: bool,

    /// Run only the Team category (knowledge distribution, activity, ownership, silos, merges)
    #[arg(long, help_heading = "Category Filters")]
    pub team: bool,

    /// Run only the Evolution category (growth, refactoring ratio, code age, cadence)
    #[arg(long, help_heading = "Category Filters")]
    pub evolution: bool,

    /// Run only the Git Hygiene category (message quality, history cleanliness, gitignore)
    #[arg(long, help_heading = "Category Filters")]
    pub hygiene: bool,

    /// Start of analysis window [default: 6 months ago]
    ///
    /// Accepts relative durations (3months, 30days, 1year) or
    /// ISO dates (2024-01-01).
    #[arg(long, help_heading = "Time Window")]
    pub since: Option<String>,

    /// End of analysis window [default: now]
    ///
    /// Accepts relative durations or ISO dates.
    #[arg(long, help_heading = "Time Window")]
    pub until: Option<String>,

    /// Analyze the full commit history (ignore time window)
    #[arg(long, help_heading = "Time Window")]
    pub all: bool,

    /// Output as JSON (mutually exclusive with --html)
    #[arg(long, help_heading = "Output Format")]
    pub json: bool,

    /// Output as a self-contained HTML report with interactive charts
    ///
    /// Generates a single-file HTML page with Overview, Hotspots, Coupling,
    /// Ownership, and Age tabs. No external dependencies — works offline.
    /// Mutually exclusive with --json.
    #[arg(long, help_heading = "Output Format")]
    pub html: bool,

    /// Generate an HTML report and open it in the default browser
    ///
    /// Implies --html. If no -o path is given, writes to a temporary file.
    /// Equivalent to: barad-dur analyze . --html -o report.html && xdg-open report.html
    #[arg(long, help_heading = "Output Format")]
    pub open: bool,

    /// Pretty-print JSON output (only effective with --json)
    #[arg(long, help_heading = "Output Format")]
    pub pretty: bool,

    /// Write output to a file instead of stdout
    #[arg(short, long, help_heading = "Output Format")]
    pub output: Option<PathBuf>,

    /// Increase verbosity (-v shows metrics, -vv shows raw values)
    #[arg(short, long, action = clap::ArgAction::Count, help_heading = "Output Format")]
    pub verbose: u8,

    /// Exclude files matching glob patterns from analysis
    ///
    /// Accepts one or more glob patterns. Files matching any pattern are
    /// excluded from all phases (blame, coupling, hotspots, complexity).
    /// Can be repeated: --exclude '*.resx' --exclude 'i18n/**'
    #[arg(long, help_heading = "Filtering", action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Disable built-in exclusion of translation/resource files
    ///
    /// By default, files matching common translation patterns are excluded:
    /// *.resx, *.po, *.pot, *.xlf, *.xliff, *.strings, *.arb, *.lproj
    #[arg(long, help_heading = "Filtering", num_args = 0..=1, default_missing_value = "true")]
    pub no_default_excludes: Option<bool>,

    /// Skip git blame (the slowest phase) for a faster partial analysis
    ///
    /// Blame-dependent metrics (bus factor, knowledge distribution, ownership,
    /// collaboration patterns, code age) will show default scores.
    /// Run again without this flag to get the full report.
    #[arg(long, help_heading = "Performance", num_args = 0..=1, default_missing_value = "true")]
    pub skip_blame: Option<bool>,

    /// Skip cache and force full re-collection from git
    #[arg(long, help_heading = "Cache")]
    pub no_cache: bool,

    /// Only use cached data; fail if no cache exists
    #[arg(long, help_heading = "Cache")]
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
        assert_eq!(args.target, ".");
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
    fn html_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--html"]);
        assert!(args.html);
        assert!(!args.json);
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
    fn open_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--open"]);
        assert!(args.open);
    }

    #[test]
    fn open_with_output() {
        let args = parse(&["barad-dur", "analyze", ".", "--open", "-o", "report.html"]);
        assert!(args.open);
        assert_eq!(args.output, Some(PathBuf::from("report.html")));
    }

    #[test]
    fn skip_blame_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--skip-blame"]);
        assert_eq!(args.skip_blame, Some(true));
    }

    #[test]
    fn skip_blame_absent() {
        let args = parse(&["barad-dur", "analyze", "."]);
        assert_eq!(args.skip_blame, None);
    }

    #[test]
    fn exclude_flag_single() {
        let args = parse(&["barad-dur", "analyze", ".", "--exclude", "*.resx"]);
        assert_eq!(args.exclude, vec!["*.resx"]);
        assert_eq!(args.no_default_excludes, None);
    }

    #[test]
    fn exclude_flag_multiple() {
        let args = parse(&[
            "barad-dur",
            "analyze",
            ".",
            "--exclude",
            "*.resx",
            "--exclude",
            "**/i18n/**",
        ]);
        assert_eq!(args.exclude, vec!["*.resx", "**/i18n/**"]);
    }

    #[test]
    fn no_default_excludes_flag() {
        let args = parse(&["barad-dur", "analyze", ".", "--no-default-excludes"]);
        assert_eq!(args.no_default_excludes, Some(true));
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
