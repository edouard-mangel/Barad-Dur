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

#[cfg(test)]
mod tests {
    use barad_dur::runner::{build_time_window_from_config, parse_time_spec};
    use chrono::Utc;

    fn parse_relative(spec: &str, suffixes: &[&str], days_per_unit: i64) -> Option<i64> {
        suffixes
            .iter()
            .find_map(|s| spec.strip_suffix(s))
            .and_then(|n| n.trim().parse::<i64>().ok())
            .map(|n| n * days_per_unit)
    }

    // --- parse_relative ---

    #[test]
    fn parse_relative_plural_suffix() {
        assert_eq!(
            parse_relative("3months", &["months", "month"], 30),
            Some(90)
        );
    }

    #[test]
    fn parse_relative_singular_suffix() {
        assert_eq!(parse_relative("1month", &["months", "month"], 30), Some(30));
    }

    #[test]
    fn parse_relative_days() {
        assert_eq!(parse_relative("30days", &["days", "day"], 1), Some(30));
        assert_eq!(parse_relative("1day", &["days", "day"], 1), Some(1));
    }

    #[test]
    fn parse_relative_years() {
        assert_eq!(parse_relative("2years", &["years", "year"], 365), Some(730));
        assert_eq!(parse_relative("1year", &["years", "year"], 365), Some(365));
    }

    #[test]
    fn parse_relative_trims_whitespace() {
        assert_eq!(
            parse_relative("3 months", &["months", "month"], 30),
            Some(90)
        );
    }

    #[test]
    fn parse_relative_non_numeric_returns_none() {
        assert_eq!(parse_relative("fewmonths", &["months", "month"], 30), None);
    }

    #[test]
    fn parse_relative_no_matching_suffix_returns_none() {
        assert_eq!(parse_relative("3years", &["months", "month"], 30), None);
    }

    // --- parse_time_spec ---

    #[test]
    fn parse_time_spec_months() {
        let now = Utc::now();
        let result = parse_time_spec("6months", now).unwrap();
        let diff = (now - result).num_days();
        assert!(
            (179..=181).contains(&diff),
            "6months should be ~180 days, got {diff}"
        );
    }

    #[test]
    fn parse_time_spec_days() {
        let now = Utc::now();
        let result = parse_time_spec("30days", now).unwrap();
        let diff = (now - result).num_days();
        assert_eq!(diff, 30);
    }

    #[test]
    fn parse_time_spec_years() {
        let now = Utc::now();
        let result = parse_time_spec("1year", now).unwrap();
        let diff = (now - result).num_days();
        assert_eq!(diff, 365);
    }

    #[test]
    fn parse_time_spec_iso_date() {
        let now = Utc::now();
        let result = parse_time_spec("2024-01-15", now).unwrap();
        assert_eq!(result.format("%Y-%m-%d").to_string(), "2024-01-15");
    }

    #[test]
    fn parse_time_spec_invalid_returns_none() {
        let now = Utc::now();
        assert!(parse_time_spec("not-a-date", now).is_none());
    }
}
