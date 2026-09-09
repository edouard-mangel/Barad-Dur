use std::path::PathBuf;

use anyhow::{bail, Result};
use barad_dur::report_contract::export_report_fixture;

const DEFAULT_OUTPUT: &str = "tests/fixtures/report-contract/current.ts";

fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let output = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    if let Some(argument) = arguments.next() {
        bail!("unexpected argument: {argument}");
    }

    export_report_fixture(&output)
}
