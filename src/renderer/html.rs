use anyhow::Result;
use crate::scorer::AnalysisReport;

pub fn render(_report: &AnalysisReport) -> Result<String> {
    Ok(String::from("<!-- TODO -->"))
}
