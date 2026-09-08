use std::path::PathBuf;

use anyhow::{bail, Result};
use barad_dur::report_contract::{check_report_types, export_report_types};

const DEFAULT_OUTPUT: &str = "dashboard/src/report/generated";

fn main() -> Result<()> {
    let mut check = false;
    let mut output = None;

    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--check" => check = true,
            value if output.is_none() => output = Some(PathBuf::from(value)),
            value => bail!("unexpected argument: {value}"),
        }
    }
    let output = output.unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));

    if check {
        check_report_types(&output)
    } else {
        export_report_types(&output)
    }
}
