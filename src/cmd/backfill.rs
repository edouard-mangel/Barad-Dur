use anyhow::Result;
use std::path::PathBuf;

use crate::backfill;
use crate::cli::BackfillArgs;

pub fn run_backfill(args: BackfillArgs) -> Result<()> {
    let repo_path = PathBuf::from(&args.target);
    backfill::run(&args, &repo_path)
}
