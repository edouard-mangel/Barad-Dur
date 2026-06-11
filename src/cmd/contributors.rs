use anyhow::Result;

use crate::cli::ContributorsArgs;
use crate::contributors;

pub fn run_contributors(args: ContributorsArgs) -> Result<()> {
    contributors::run(&args)
}
