use anyhow::Result;
use std::path::PathBuf;

use crate::cli::InitArgs;
use crate::init;

pub fn run_init(args: InitArgs) -> Result<()> {
    let target = PathBuf::from(&args.target);
    init::run_init(
        &target,
        init::InitOptions {
            force: args.force,
            interactive: args.interactive,
        },
    )
}
